//! Secondary verification for sensitive actions (P7-18…P7-22).
//!
//! Windows uses the native credential prompt and validates the entered account
//! with `LogonUserW`. Other platforms report `unsupported` so the UI can explain
//! the limitation instead of pretending verification happened. Windows Hello is
//! not wired yet; the native credential prompt is the fallback described by
//! P7-19.

use hr_core::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub verified: bool,
    pub supported: bool,
    pub message: Option<String>,
}

impl VerificationResult {
    pub fn unsupported() -> Self {
        Self {
            verified: false,
            supported: false,
            message: Some(
                "Secondary verification is only available on Windows in this build.".to_string(),
            ),
        }
    }
}

#[cfg(windows)]
mod ffi {
    use core::ffi::c_void;

    #[repr(C)]
    pub struct CredUiInfo {
        pub cb_size: u32,
        pub hwnd_parent: *mut c_void,
        pub message_text: *const u16,
        pub caption_text: *const u16,
        pub banner: *mut c_void,
    }

    #[link(name = "credui")]
    unsafe extern "system" {
        pub fn CredUIPromptForWindowsCredentialsW(
            info: *const CredUiInfo,
            auth_error: u32,
            auth_package: *mut u32,
            in_buffer: *const c_void,
            in_size: u32,
            out_buffer: *mut *mut c_void,
            out_size: *mut u32,
            save: *mut i32,
            flags: u32,
        ) -> u32;
        pub fn CredUnPackAuthenticationBufferW(
            flags: u32,
            auth_buffer: *const c_void,
            auth_size: u32,
            user_name: *mut u16,
            user_name_size: *mut u32,
            domain: *mut u16,
            domain_size: *mut u32,
            password: *mut u16,
            password_size: *mut u32,
        ) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn LogonUserW(
            user: *const u16,
            domain: *const u16,
            password: *const u16,
            logon_type: u32,
            provider: u32,
            token: *mut *mut c_void,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn CloseHandle(handle: *mut c_void) -> i32;
        pub fn GetLastError() -> u32;
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        pub fn CoTaskMemFree(pointer: *mut c_void);
    }
}

#[cfg(windows)]
pub fn verify_user(message: &str) -> AppResult<VerificationResult> {
    use std::ffi::c_void;

    use ffi::{
        CloseHandle, CoTaskMemFree, CredUIPromptForWindowsCredentialsW, CredUiInfo,
        CredUnPackAuthenticationBufferW, LogonUserW,
    };

    const CREDUIWIN_GENERIC: u32 = 0x1;
    const ERROR_CANCELLED: u32 = 1223;
    const ERROR_LOGON_FAILURE: u32 = 1326;
    const LOGON32_LOGON_INTERACTIVE: u32 = 2;
    const LOGON32_PROVIDER_DEFAULT: u32 = 0;

    let wide =
        |value: &str| -> Vec<u16> { value.encode_utf16().chain(std::iter::once(0)).collect() };
    let caption = wide("H-Remote");
    let message = wide(message);
    let info = CredUiInfo {
        cb_size: std::mem::size_of::<CredUiInfo>() as u32,
        hwnd_parent: std::ptr::null_mut(),
        message_text: message.as_ptr(),
        caption_text: caption.as_ptr(),
        banner: std::ptr::null_mut(),
    };
    let mut auth_error = 0u32;
    for _ in 0..3 {
        let mut package = 0u32;
        let mut out: *mut c_void = std::ptr::null_mut();
        let mut out_size = 0u32;
        let mut save = 0i32;
        let code = unsafe {
            CredUIPromptForWindowsCredentialsW(
                &info,
                auth_error,
                &mut package,
                std::ptr::null(),
                0,
                &mut out,
                &mut out_size,
                &mut save,
                CREDUIWIN_GENERIC,
            )
        };
        if code == ERROR_CANCELLED {
            return Ok(VerificationResult {
                verified: false,
                supported: true,
                message: Some("Verification was cancelled.".to_string()),
            });
        }
        if code != 0 {
            return Ok(VerificationResult {
                verified: false,
                supported: true,
                message: Some(format!("The credential prompt failed (error {code}).")),
            });
        }
        let mut user = vec![0u16; 256];
        let mut domain = vec![0u16; 256];
        let mut password = vec![0u16; 256];
        let mut user_len = 256u32;
        let mut domain_len = 256u32;
        let mut password_len = 256u32;
        let unpacked = unsafe {
            CredUnPackAuthenticationBufferW(
                0,
                out,
                out_size,
                user.as_mut_ptr(),
                &mut user_len,
                domain.as_mut_ptr(),
                &mut domain_len,
                password.as_mut_ptr(),
                &mut password_len,
            )
        };
        unsafe { CoTaskMemFree(out) };
        if unpacked == 0 {
            auth_error = unsafe { ffi::GetLastError() };
            continue;
        }
        let mut token: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            LogonUserW(
                user.as_ptr(),
                domain.as_ptr(),
                password.as_ptr(),
                LOGON32_LOGON_INTERACTIVE,
                LOGON32_PROVIDER_DEFAULT,
                &mut token,
            )
        };
        let last_error = unsafe { ffi::GetLastError() };
        if !token.is_null() {
            unsafe {
                CloseHandle(token);
            }
        }
        // Best-effort wipe of the UTF-16 password buffer.
        password.fill(0);
        if ok != 0 {
            return Ok(VerificationResult {
                verified: true,
                supported: true,
                message: None,
            });
        }
        if last_error != ERROR_LOGON_FAILURE {
            return Ok(VerificationResult {
                verified: false,
                supported: true,
                message: Some(format!(
                    "Windows rejected the credentials (error {last_error})."
                )),
            });
        }
        auth_error = ERROR_LOGON_FAILURE;
    }
    Ok(VerificationResult {
        verified: false,
        supported: true,
        message: Some("Too many failed verification attempts.".to_string()),
    })
}

#[cfg(not(windows))]
pub fn verify_user(_message: &str) -> AppResult<VerificationResult> {
    Ok(VerificationResult::unsupported())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_camel_case() {
        let result = VerificationResult {
            verified: false,
            supported: true,
            message: Some("nope".into()),
        };
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["supported"], true);
        assert_eq!(json["message"], "nope");
    }
}
