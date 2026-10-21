//! 建索引時不干擾使用者：依時段決定用幾個執行緒、要不要背景執行，什麼情況下先暫停。
//!
//! - 夜間：全速，執行緒數等於效能核心數，不用背景模式，並防止電腦睡眠（由呼叫端處理）。
//! - 白天：最多 `day_threads` 個執行緒，背景模式（低優先權、效率模式）。
//! - 可用記憶體太少：暫停，並卸載建索引用的模型。白天、夜間都一樣。
//! - 白天別的程式讓 CPU 很忙：暫停，等忙完再繼續。夜間不看 CPU（使用者通常不在）。
//!
//! 暫停和恢復的門檻不同（恢復要等到明顯回落），避免在門檻附近一直切換。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThrottleSettings {
    /// 夜間從幾點開始、到幾點結束（0–23；開始大於結束表示跨過午夜）。
    pub night_start: u8,
    pub night_end: u8,
    /// 白天最多用幾個執行緒。
    pub day_threads: usize,
    /// 白天別的程式用掉的 CPU 比例超過這個值就暫停，降到 `cpu_resume` 以下才繼續。
    pub cpu_pause: f64,
    pub cpu_resume: f64,
    /// 可用記憶體少於這個值（位元組）就暫停，回到 `memory_resume` 以上才繼續。
    pub memory_pause: u64,
    pub memory_resume: u64,
}

impl Default for ThrottleSettings {
    fn default() -> Self {
        Self {
            night_start: 20,
            night_end: 7,
            day_threads: 2,
            cpu_pause: 0.5,
            cpu_resume: 0.3,
            memory_pause: 1 << 30,
            memory_resume: 3 << 29,
        }
    }
}

impl ThrottleSettings {
    pub fn is_night(&self, hour: u8) -> bool {
        if self.night_start == self.night_end {
            return false;
        }
        if self.night_start < self.night_end {
            (self.night_start..self.night_end).contains(&hour)
        } else {
            hour >= self.night_start || hour < self.night_end
        }
    }
}

/// 暫停的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PauseReason {
    /// 使用者按了暫停。
    User,
    /// 可用記憶體不足。
    Memory,
    /// 白天別的程式正在大量使用 CPU。
    Busy,
}

impl PauseReason {
    pub fn describe(self) -> &'static str {
        match self {
            PauseReason::User => "已暫停",
            PauseReason::Memory => "記憶體不足，暫停中",
            PauseReason::Busy => "電腦正忙，暫停中",
        }
    }
}

/// 這一刻的狀況。
#[derive(Debug, Clone, Copy)]
pub struct Conditions {
    pub hour: u8,
    /// 使用者按了暫停。
    pub user_paused: bool,
    /// 可用記憶體（位元組）；讀不到時 `None`，當作足夠。
    pub available_memory: Option<u64>,
    /// 別的程式用掉的 CPU 比例；還沒量到時 `None`，當作不忙。
    pub others_busy: Option<f64>,
    /// 效能核心數。
    pub performance_cores: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    Run { threads: usize, background: bool, night: bool },
    Pause(PauseReason),
}

/// 決定接下來要怎麼做。`paused` 是上一次的決定是否為暫停及原因（用來套用恢復的門檻）。
pub fn decide(settings: &ThrottleSettings, now: &Conditions, paused: Option<PauseReason>) -> Decision {
    if now.user_paused {
        return Decision::Pause(PauseReason::User);
    }
    if let Some(available) = now.available_memory {
        let limit = if paused == Some(PauseReason::Memory) { settings.memory_resume } else { settings.memory_pause };
        if available < limit {
            return Decision::Pause(PauseReason::Memory);
        }
    }
    let night = settings.is_night(now.hour);
    if !night {
        if let Some(busy) = now.others_busy {
            let limit = if paused == Some(PauseReason::Busy) { settings.cpu_resume } else { settings.cpu_pause };
            if busy > limit {
                return Decision::Pause(PauseReason::Busy);
            }
        }
    }
    let cores = now.performance_cores.max(1);
    let threads = if night { cores } else { settings.day_threads.clamp(1, cores) };
    Decision::Run { threads, background: !night, night }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u8) -> Conditions {
        Conditions { hour, user_paused: false, available_memory: Some(4 << 30), others_busy: Some(0.1), performance_cores: 6 }
    }

    #[test]
    fn night_window_wraps_midnight() {
        let s = ThrottleSettings::default();
        assert!(s.is_night(20) && s.is_night(23) && s.is_night(0) && s.is_night(6));
        assert!(!s.is_night(7) && !s.is_night(12) && !s.is_night(19));
        let same_day = ThrottleSettings { night_start: 1, night_end: 5, ..s };
        assert!(same_day.is_night(1) && !same_day.is_night(5) && !same_day.is_night(23));
    }

    #[test]
    fn day_is_throttled_and_night_is_full_speed() {
        let s = ThrottleSettings::default();
        assert_eq!(decide(&s, &at(10), None), Decision::Run { threads: 2, background: true, night: false });
        assert_eq!(decide(&s, &at(22), None), Decision::Run { threads: 6, background: false, night: true });
        let one_core = Conditions { performance_cores: 1, ..at(10) };
        assert_eq!(decide(&s, &one_core, None), Decision::Run { threads: 1, background: true, night: false });
    }

    #[test]
    fn pauses_with_hysteresis() {
        let s = ThrottleSettings::default();
        let busy = Conditions { others_busy: Some(0.6), ..at(10) };
        assert_eq!(decide(&s, &busy, None), Decision::Pause(PauseReason::Busy));
        // 降到 0.4：還沒低於恢復門檻 0.3，繼續暫停；沒在暫停的話 0.4 不會觸發
        let calmer = Conditions { others_busy: Some(0.4), ..at(10) };
        assert_eq!(decide(&s, &calmer, Some(PauseReason::Busy)), Decision::Pause(PauseReason::Busy));
        assert!(matches!(decide(&s, &calmer, None), Decision::Run { .. }));
        // 夜間不看 CPU
        let busy_night = Conditions { others_busy: Some(0.9), ..at(23) };
        assert!(matches!(decide(&s, &busy_night, None), Decision::Run { .. }));
        // 記憶體不足：日夜都暫停；使用者暫停最優先
        let low = Conditions { available_memory: Some(512 << 20), ..at(23) };
        assert_eq!(decide(&s, &low, None), Decision::Pause(PauseReason::Memory));
        let user = Conditions { user_paused: true, ..low };
        assert_eq!(decide(&s, &user, None), Decision::Pause(PauseReason::User));
    }
}
