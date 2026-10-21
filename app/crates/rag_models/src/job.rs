//! Windows 的 Job Object：把子行程放進一個 job，主程式結束時（包括當掉或被強制結束）
//! 系統會把 job 裡的行程一起結束。沒有這個的話，llama-server 會留下來繼續佔記憶體。

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

type Handle = *mut c_void;

const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

// 和 Windows SDK 的 JOBOBJECT_BASIC_LIMIT_INFORMATION / JOBOBJECT_EXTENDED_LIMIT_INFORMATION 相同的排列
#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Default)]
struct ExtendedLimits {
    basic: BasicLimits,
    io_counters: [u64; 6],
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, info: *const c_void, length: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
}

/// 持有 job 的代碼。這個值被丟掉（或主程式結束，代碼被系統收回）時，job 裡的行程就會被結束。
pub struct KillOnExit(Handle);

// 代碼只是一個系統資源的編號，可以交給別的執行緒
unsafe impl Send for KillOnExit {}

impl KillOnExit {
    /// 把子行程放進新的 job。失敗時回傳 `None`：子行程照常執行，只是少了這層保護。
    pub fn attach(child: &Child) -> Option<Self> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let job = Self(job);
            let mut limits = ExtendedLimits::default();
            limits.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of::<ExtendedLimits>() as u32;
            let ok = SetInformationJobObject(job.0, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION, (&limits as *const ExtendedLimits).cast(), size) != 0
                && AssignProcessToJobObject(job.0, child.as_raw_handle().cast()) != 0;
            ok.then_some(job)
        }
    }
}

impl Drop for KillOnExit {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_sizes_match_the_windows_sdk() {
        assert_eq!(std::mem::size_of::<BasicLimits>(), 64);
        assert_eq!(std::mem::size_of::<ExtendedLimits>(), 144);
    }
}
