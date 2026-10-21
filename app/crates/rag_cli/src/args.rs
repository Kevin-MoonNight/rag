//! 很簡單的命令列參數解析：`--名稱 值`、`--開關`、其餘是位置參數。

use anyhow::{bail, Context, Result};

pub struct Args {
    items: Vec<Option<String>>,
}

impl Args {
    pub fn parse(items: Vec<String>) -> Self {
        Self { items: items.into_iter().map(Some).collect() }
    }

    fn find(&self, name: &str) -> Option<usize> {
        self.items.iter().position(|a| a.as_deref() == Some(name))
    }

    /// 有沒有這個開關。
    pub fn flag(&mut self, name: &str) -> bool {
        match self.find(name) {
            Some(i) => {
                self.items[i] = None;
                true
            }
            None => false,
        }
    }

    /// `--名稱 值` 的值。
    pub fn value(&mut self, name: &str) -> Option<String> {
        let i = self.find(name)?;
        self.items[i] = None;
        self.items.get_mut(i + 1).and_then(Option::take)
    }

    pub fn number(&mut self, name: &str) -> Result<Option<usize>> {
        match self.value(name) {
            Some(v) => Ok(Some(v.parse().with_context(|| format!("{name} 要是整數"))?)),
            None => Ok(None),
        }
    }

    /// 第 n 個位置參數（不是 `-` 開頭、也還沒被當成某個參數的值拿走的）。
    /// 要在所有 `value()` 都呼叫過之後再用，否則參數的值會被誤認成位置參數。
    pub fn positional(&mut self, n: usize) -> Option<String> {
        self.items.iter().flatten().filter(|a| !a.starts_with('-') || a.chars().nth(1).is_some_and(|c| c.is_ascii_digit())).nth(n).cloned()
    }

    /// 檢查有沒有打錯、沒被用到的參數。
    pub fn finish(&mut self) -> Result<()> {
        let unknown: Vec<&String> = self.items.iter().flatten().filter(|a| a.starts_with("--")).collect();
        if !unknown.is_empty() {
            bail!("不認得的參數：{unknown:?}");
        }
        Ok(())
    }
}
