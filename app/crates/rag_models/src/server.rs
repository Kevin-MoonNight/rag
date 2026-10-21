//! llama-server 子行程：啟動、等它就緒、送請求、結束。

use crate::MEDIA_MARKER;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs::File;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// 子行程的用途，決定啟動參數。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMode {
    /// 向量模型（EmbeddingGemma 2）。
    Embedding,
    /// 產生回答的模型（Gemma 4 E2B）。
    Generation,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub mode: ServerMode,
    /// llama-server 執行檔。
    pub server_exe: PathBuf,
    /// 模型檔（GGUF）。
    pub model: PathBuf,
    /// 影像編碼器（GGUF）。只有建索引要處理圖片時才給；查詢時不載入，可以少用約 500 MB。
    pub mmproj: Option<PathBuf>,
    /// 產生回答時用來推測解碼的草稿模型（Gemma 4 的 MTP）。回答和不用時相同，只影響速度。
    pub draft: Option<PathBuf>,
    pub threads: usize,
    /// 背景執行（白天建索引時）：低優先權、不忙等、Windows 上設成效率模式。
    pub background: bool,
    /// 一次請求最多幾個 token。
    pub context: usize,
    /// 子行程的輸出寫到哪個檔案；不給就丟掉。
    pub log_file: Option<PathBuf>,
    /// 等模型載入完成最多等多久。
    pub startup_timeout: Duration,
    /// 單一請求最多等多久。
    pub request_timeout: Duration,
}

impl ServerConfig {
    pub fn new(server_exe: impl Into<PathBuf>, model: impl Into<PathBuf>) -> Self {
        Self {
            mode: ServerMode::Embedding,
            server_exe: server_exe.into(),
            model: model.into(),
            mmproj: None,
            draft: None,
            threads: 6,
            background: false,
            context: 8192,
            log_file: None,
            startup_timeout: Duration::from_secs(180),
            request_timeout: Duration::from_secs(600),
        }
    }

    /// 產生回答用的設定。
    pub fn generation(server_exe: impl Into<PathBuf>, model: impl Into<PathBuf>) -> Self {
        Self { mode: ServerMode::Generation, request_timeout: Duration::from_secs(1800), ..Self::new(server_exe, model) }
    }
}

/// 每次啟動子行程時產生的金鑰。子行程只聽本機位址，但同一台電腦上的其他程式仍然連得到，
/// 有金鑰才能使用。
fn random_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    // RandomState 每次建立都用系統亂數當種子
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

pub struct LlamaServer {
    child: Child,
    port: u16,
    agent: ureq::Agent,
    key: String,
    log_file: Option<PathBuf>,
    /// 主程式不正常結束時，讓系統把子行程一起結束。
    #[cfg(windows)]
    _job: Option<crate::job::KillOnExit>,
}

impl LlamaServer {
    /// 啟動子行程並等到它可以接受請求。只聽本機的位址。
    pub fn start(config: &ServerConfig) -> Result<Self> {
        // 向系統要一個目前沒人用的埠號
        let port = TcpListener::bind(("127.0.0.1", 0)).context("找不到可用的本機埠號")?.local_addr()?.port();

        let key = random_key();
        let mut cmd = Command::new(&config.server_exe);
        cmd.env("LLAMA_MEDIA_MARKER", MEDIA_MARKER)
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--api-key", &key, "-m"])
            .arg(&config.model)
            .args(["-t", &config.threads.to_string(), "-c", &config.context.to_string()])
            .args(["-lm", "none", "-lzm", "on", "--cache-ram", "0", "-ngl", "0", "--no-webui", "--parallel", "1"]);
        match config.mode {
            ServerMode::Embedding => {
                cmd.args(["--embedding", "-b", "1024", "-ub", "1024", "--image-min-tokens", "280", "--image-max-tokens", "280"]);
            }
            // 關掉思考模式：CPU 上每秒只產生二十幾個 token，先想幾百個 token 會多等很久
            ServerMode::Generation => {
                cmd.args(["--reasoning", "off"]);
                if let Some(draft) = &config.draft {
                    cmd.arg("-md").arg(draft).args(["--spec-type", "draft-mtp"]);
                }
            }
        }
        if config.background {
            // 低優先權；工作之間不忙等（預設 --poll 50 會在等待時空轉吃 CPU）
            cmd.args(["--prio", "-1", "--poll", "0"]);
        }
        if let Some(mmproj) = &config.mmproj {
            cmd.arg("--mmproj").arg(mmproj);
        }
        cmd.stdin(Stdio::null());
        match &config.log_file {
            Some(path) => {
                let file = File::create(path).with_context(|| format!("無法建立紀錄檔 {}", path.display()))?;
                cmd.stdout(file.try_clone()?).stderr(file);
            }
            None => {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW：不要跳出主控台視窗
        }
        let child = cmd.spawn().with_context(|| format!("無法啟動 {}", config.server_exe.display()))?;
        if config.background {
            crate::system::set_efficiency_mode(&child, true);
        }

        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(config.request_timeout)).http_status_as_error(false).build().into();
        #[cfg(windows)]
        let job = crate::job::KillOnExit::attach(&child);
        CHILDREN.lock().unwrap_or_else(|p| p.into_inner()).push(child.id());
        let mut server = Self {
            child,
            port,
            agent,
            key,
            log_file: config.log_file.clone(),
            #[cfg(windows)]
            _job: job,
        };

        let started = Instant::now();
        loop {
            if let Some(status) = server.child.try_wait()? {
                bail!("llama-server 啟動後就結束了（{status}）{}", server.log_tail());
            }
            if server.healthy() {
                return Ok(server);
            }
            if started.elapsed() > config.startup_timeout {
                bail!("llama-server 在 {} 秒內沒有就緒{}", config.startup_timeout.as_secs(), server.log_tail());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn healthy(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/health", self.port);
        match self.agent.get(&url).config().timeout_global(Some(Duration::from_secs(2))).build().call() {
            Ok(resp) => resp.status().as_u16() == 200,
            Err(_) => false,
        }
    }

    fn send(&self, path: &str, payload: &Value) -> Result<ureq::http::Response<ureq::Body>> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        self.agent
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.key))
            .send_json(payload)
            .with_context(|| format!("向 llama-server 送出 {path} 失敗"))
    }

    pub fn post_json(&self, path: &str, payload: &Value) -> Result<Value> {
        let mut resp = self.send(path, payload)?;
        let status = resp.status().as_u16();
        let body = resp.body_mut().with_config().limit(256 * 1024 * 1024).read_to_string().context("讀取 llama-server 的回應失敗")?;
        if status != 200 {
            bail!("llama-server 回應 {status}：{}", body.chars().take(300).collect::<String>());
        }
        serde_json::from_str(&body).context("llama-server 的回應不是 JSON")
    }

    /// 送出請求，回應是一連串的 `data: {...}` 行（串流）。每收到一段就交給 `on_event`；
    /// `on_event` 回傳 `false` 時不再讀下去，連線關掉後 llama-server 會停止產生。
    pub fn post_stream(&self, path: &str, payload: &Value, on_event: &mut dyn FnMut(&Value) -> bool) -> Result<()> {
        use std::io::BufRead;
        let resp = self.send(path, payload)?;
        let status = resp.status().as_u16();
        let mut body = resp.into_body();
        if status != 200 {
            let text = body.with_config().limit(1024 * 1024).read_to_string().unwrap_or_default();
            bail!("llama-server 回應 {status}：{}", text.chars().take(300).collect::<String>());
        }
        let reader = std::io::BufReader::new(body.into_reader());
        for line in reader.lines() {
            let line = line.context("讀取 llama-server 的串流回應失敗")?;
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let value: Value = serde_json::from_str(data).with_context(|| format!("串流回應不是 JSON：{}", data.chars().take(200).collect::<String>()))?;
            if let Some(error) = value.get("error") {
                bail!("llama-server 回報錯誤：{}", error.to_string().chars().take(300).collect::<String>());
            }
            if !on_event(&value) {
                break;
            }
        }
        Ok(())
    }

    fn log_tail(&self) -> String {
        let Some(path) = &self.log_file else { return String::new() };
        let Ok(text) = std::fs::read_to_string(path) else { return String::new() };
        let lines: Vec<&str> = text.lines().collect();
        format!("\n紀錄檔最後幾行：\n{}", lines[lines.len().saturating_sub(15)..].join("\n"))
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        let pid = self.child.id();
        let _ = self.child.kill();
        let _ = self.child.wait();
        CHILDREN.lock().unwrap_or_else(|p| p.into_inner()).retain(|&p| p != pid);
    }
}

/// 目前在執行的 llama-server 子行程。量「別的程式有多忙」時要扣掉它們用的 CPU。
static CHILDREN: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

/// 這個主程式和它目前所有的 llama-server 子行程的編號。
pub fn own_pids() -> Vec<u32> {
    let mut pids = vec![std::process::id()];
    pids.extend(CHILDREN.lock().unwrap_or_else(|p| p.into_inner()).iter().copied());
    pids
}
