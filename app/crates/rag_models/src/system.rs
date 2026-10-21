//! 和作業系統打交道的小工具：記憶體、CPU 忙不忙、核心數，以及讓建索引不干擾使用者的設定
//! （背景模式、效率模式、防止睡眠）。主程式結束時一併結束子程序的部分在 `job.rs`。
//!
//! Windows 以外（開發用的 Linux 容器）只實作讀數字的部分，設定類的函式什麼都不做。

use std::process::Child;

/// 實體記憶體：總量和目前可用的量（位元組）。拿不到時回傳 `None`。
pub fn memory() -> Option<(u64, u64)> {
    imp::memory()
}

/// 整台電腦累計的 CPU 時間（秒，所有核心加總）：忙碌的和全部的。取兩次相減就知道這段時間有多忙。
pub fn system_cpu_times() -> Option<(f64, f64)> {
    imp::system_cpu_times()
}

/// 一個程序累計用掉的 CPU 時間（秒）。程序不存在時回傳 `None`。
pub fn process_cpu_seconds(pid: u32) -> Option<f64> {
    imp::process_cpu_seconds(pid)
}

/// 這個程序到目前為止的記憶體高峰（位元組）：Windows 是私有記憶體（commit）的高峰，Linux 是常駐記憶體的高峰。
pub fn peak_memory() -> Option<u64> {
    imp::peak_memory()
}

/// 實體核心數，和其中效能核心的數目（大小核的 CPU；沒有大小核時兩者相同）。
pub fn physical_cores() -> Option<(usize, usize)> {
    imp::physical_cores()
}

/// 讓目前的執行緒進入（或離開）背景模式：CPU、磁碟和記憶體的優先權都降低。
/// 微軟的文件說只降 CPU 優先權不夠，磁碟和記憶體的使用一樣會干擾前景程式。
pub fn set_current_thread_background(on: bool) {
    imp::set_current_thread_background(on)
}

/// 存在期間防止電腦自動睡眠（螢幕可以關）。要在同一個執行緒建立和結束。
/// 筆電用電池時，系統仍會在幾分鐘後讓它睡；使用者自己按睡眠也擋不住。
pub struct KeepAwake(());

impl KeepAwake {
    pub fn new() -> Self {
        imp::keep_awake(true);
        KeepAwake(())
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        imp::keep_awake(false);
    }
}

/// 子程序設成效率模式（Windows 11 的 EcoQoS：排在省電的核心、用較低的時脈）。
pub fn set_efficiency_mode(child: &Child, on: bool) {
    imp::set_efficiency_mode(child, on)
}


/// 量「別的程式」讓 CPU 有多忙：整台電腦的忙碌時間扣掉自己（主程式和模型子程序）用掉的。
#[derive(Debug, Default)]
pub struct CpuMonitor {
    last: Option<(f64, f64, f64)>,
}

impl CpuMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// 和上一次呼叫之間，別的程式用掉的 CPU 比例（0 到 1）。第一次呼叫只記錄起點，回傳 `None`。
    /// 自己這邊（主程式和所有 llama-server 子行程）用掉的不算。
    pub fn others_busy(&mut self) -> Option<f64> {
        let (busy, total) = system_cpu_times()?;
        let own: f64 = crate::server::own_pids().iter().filter_map(|&pid| process_cpu_seconds(pid)).sum();
        let previous = self.last.replace((busy, total, own));
        let (b0, t0, o0) = previous?;
        let elapsed = total - t0;
        if elapsed <= 0.0 {
            return None;
        }
        // 子程序中途換過（重新載入）時自己的時間會倒退，這種情況只扣掉不倒退的部分
        let own_delta = (own - o0).max(0.0);
        Some(((busy - b0 - own_delta) / elapsed).clamp(0.0, 1.0))
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    type Handle = *mut c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        /// 100 奈秒為單位 → 秒
        fn seconds(self) -> f64 {
            (((self.high as u64) << 32) | self.low as u64) as f64 / 1e7
        }
    }

    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[repr(C)]
    struct PowerThrottlingState {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn K32GetProcessMemoryInfo(process: Handle, counters: *mut ProcessMemoryCounters, size: u32) -> i32;
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetProcessTimes(process: Handle, creation: *mut FileTime, exit: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GetCurrentThread() -> Handle;
        fn SetThreadPriority(thread: Handle, priority: i32) -> i32;
        fn SetThreadExecutionState(flags: u32) -> u32;
        fn SetProcessInformation(process: Handle, class: i32, info: *mut c_void, size: u32) -> i32;
        fn GetLogicalProcessorInformationEx(relationship: i32, buffer: *mut u8, length: *mut u32) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const THREAD_MODE_BACKGROUND_BEGIN: i32 = 0x0001_0000;
    const THREAD_MODE_BACKGROUND_END: i32 = 0x0002_0000;
    const ES_CONTINUOUS: u32 = 0x8000_0000;
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const PROCESS_POWER_THROTTLING: i32 = 4;
    const POWER_THROTTLING_EXECUTION_SPEED: u32 = 0x1;
    const RELATION_PROCESSOR_CORE: i32 = 0;

    pub fn memory() -> Option<(u64, u64)> {
        let mut status = MemoryStatusEx {
            length: std::mem::size_of::<MemoryStatusEx>() as u32,
            memory_load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page_file: 0,
            avail_page_file: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended_virtual: 0,
        };
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) } != 0;
        ok.then_some((status.total_phys, status.avail_phys))
    }

    pub fn system_cpu_times() -> Option<(f64, f64)> {
        let (mut idle, mut kernel, mut user) = (FileTime::default(), FileTime::default(), FileTime::default());
        if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return None;
        }
        // 核心時間包含閒置時間
        let total = kernel.seconds() + user.seconds();
        Some((total - idle.seconds(), total))
    }

    pub fn process_cpu_seconds(pid: u32) -> Option<f64> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let mut t = [FileTime::default(); 4];
        let ok = unsafe { GetProcessTimes(handle, &mut t[0], &mut t[1], &mut t[2], &mut t[3]) } != 0;
        unsafe { CloseHandle(handle) };
        ok.then(|| t[2].seconds() + t[3].seconds())
    }

    pub fn peak_memory() -> Option<u64> {
        let mut counters = ProcessMemoryCounters { cb: std::mem::size_of::<ProcessMemoryCounters>() as u32, ..Default::default() };
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) } != 0;
        ok.then_some(counters.peak_pagefile_usage as u64)
    }

    pub fn physical_cores() -> Option<(usize, usize)> {
        let mut length = 0u32;
        unsafe { GetLogicalProcessorInformationEx(RELATION_PROCESSOR_CORE, std::ptr::null_mut(), &mut length) };
        if length == 0 {
            return None;
        }
        let mut buffer = vec![0u8; length as usize];
        if unsafe { GetLogicalProcessorInformationEx(RELATION_PROCESSOR_CORE, buffer.as_mut_ptr(), &mut length) } == 0 {
            return None;
        }
        // 每一筆：Relationship(4) Size(4) Flags(1) EfficiencyClass(1) …；EfficiencyClass 越大越快
        let mut classes = Vec::new();
        let mut offset = 0usize;
        while offset + 10 <= length as usize {
            let size = u32::from_le_bytes(buffer[offset + 4..offset + 8].try_into().ok()?) as usize;
            if size == 0 {
                break;
            }
            classes.push(buffer[offset + 9]);
            offset += size;
        }
        let best = *classes.iter().max()?;
        Some((classes.len(), classes.iter().filter(|&&c| c == best).count()))
    }

    pub fn set_current_thread_background(on: bool) {
        let mode = if on { THREAD_MODE_BACKGROUND_BEGIN } else { THREAD_MODE_BACKGROUND_END };
        unsafe { SetThreadPriority(GetCurrentThread(), mode) };
    }

    pub fn keep_awake(on: bool) {
        let flags = if on { ES_CONTINUOUS | ES_SYSTEM_REQUIRED } else { ES_CONTINUOUS };
        unsafe { SetThreadExecutionState(flags) };
    }

    pub fn set_efficiency_mode(child: &Child, on: bool) {
        let mut state = PowerThrottlingState {
            version: 1,
            control_mask: POWER_THROTTLING_EXECUTION_SPEED,
            state_mask: if on { POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
        };
        unsafe {
            SetProcessInformation(
                child.as_raw_handle() as Handle,
                PROCESS_POWER_THROTTLING,
                &mut state as *mut _ as *mut c_void,
                std::mem::size_of::<PowerThrottlingState>() as u32,
            )
        };
    }

}

#[cfg(not(windows))]
mod imp {
    use std::process::Child;

    /// Linux 的 CPU 時間單位（USER_HZ），一般都是 100。
    const TICKS: f64 = 100.0;

    pub fn memory() -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let field = |name: &str| -> Option<u64> {
            let line = text.lines().find(|l| l.starts_with(name))?;
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            Some(kb * 1024)
        };
        Some((field("MemTotal:")?, field("MemAvailable:")?))
    }

    pub fn system_cpu_times() -> Option<(f64, f64)> {
        let text = std::fs::read_to_string("/proc/stat").ok()?;
        let values: Vec<f64> = text.lines().next()?.split_whitespace().skip(1).filter_map(|v| v.parse().ok()).collect();
        let total: f64 = values.iter().take(8).sum();
        // 第 4、5 個是閒置和等待磁碟
        let idle = values.get(3)? + values.get(4).unwrap_or(&0.0);
        Some(((total - idle) / TICKS, total / TICKS))
    }

    pub fn process_cpu_seconds(pid: u32) -> Option<f64> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // 程式名稱可能含空白，從最後一個右括號之後開始數；utime、stime 是第 14、15 個欄位
        let rest = &text[text.rfind(')')? + 2..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let utime: f64 = fields.get(11)?.parse().ok()?;
        let stime: f64 = fields.get(12)?.parse().ok()?;
        Some((utime + stime) / TICKS)
    }

    pub fn peak_memory() -> Option<u64> {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        let kb: u64 = text.lines().find(|l| l.starts_with("VmHWM:"))?.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    }

    pub fn physical_cores() -> Option<(usize, usize)> {
        let mut cores = std::collections::HashSet::new();
        for entry in std::fs::read_dir("/sys/devices/system/cpu").ok()?.flatten() {
            let topology = entry.path().join("topology");
            let read = |name: &str| std::fs::read_to_string(topology.join(name)).ok().map(|s| s.trim().to_string());
            if let (Some(package), Some(core)) = (read("physical_package_id"), read("core_id")) {
                cores.insert((package, core));
            }
        }
        (!cores.is_empty()).then(|| (cores.len(), cores.len()))
    }

    pub fn set_current_thread_background(_on: bool) {}

    pub fn keep_awake(_on: bool) {}

    pub fn set_efficiency_mode(_child: &Child, _on: bool) {}

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_readable() {
        let (total, available) = memory().expect("讀不到記憶體資訊");
        assert!(total > 256 << 20);
        assert!(available <= total);
    }

    #[test]
    fn cpu_numbers_are_readable() {
        let (busy, total) = system_cpu_times().expect("讀不到 CPU 時間");
        assert!(busy >= 0.0 && busy <= total);
        assert!(process_cpu_seconds(std::process::id()).is_some());
        let (all, performance) = physical_cores().expect("讀不到核心數");
        assert!(all >= 1 && performance >= 1 && performance <= all);
        let mut monitor = CpuMonitor::new();
        assert!(monitor.others_busy().is_none());
    }
}
