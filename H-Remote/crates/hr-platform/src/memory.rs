use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrimOutcome {
    Trimmed,
    Unsupported,
    Failed,
}

/// Releases unused pages back to the operating system after startup (P0-54).
pub fn trim_working_set() -> TrimOutcome {
    #[cfg(windows)]
    {
        return windows_trim();
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        return linux_trim();
    }
    #[allow(unreachable_code)]
    TrimOutcome::Unsupported
}

#[cfg(windows)]
fn windows_trim() -> TrimOutcome {
    use core::ffi::c_void;
    type Handle = *mut c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> Handle;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn EmptyWorkingSet(process: Handle) -> i32;
    }

    // SAFETY: both calls operate on the current process pseudo-handle and have
    // no ownership requirements. EmptyWorkingSet returns zero on failure.
    let success = unsafe { EmptyWorkingSet(GetCurrentProcess()) != 0 };
    if success {
        TrimOutcome::Trimmed
    } else {
        TrimOutcome::Failed
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn linux_trim() -> TrimOutcome {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    // SAFETY: malloc_trim is safe to call from any thread; the return value is
    // advisory (1 = memory released, 0 = nothing to release).
    unsafe {
        malloc_trim(0);
    }
    TrimOutcome::Trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_never_panics_or_reports_a_platform_without_support_as_success() {
        let outcome = trim_working_set();
        assert_ne!(outcome, TrimOutcome::Failed);
        #[cfg(not(any(windows, all(target_os = "linux", target_env = "gnu"))))]
        assert_eq!(outcome, TrimOutcome::Unsupported);
    }

    #[test]
    fn outcome_is_serialisable() {
        assert_eq!(
            serde_json::to_string(&TrimOutcome::Trimmed).unwrap(),
            "\"trimmed\""
        );
    }
}
