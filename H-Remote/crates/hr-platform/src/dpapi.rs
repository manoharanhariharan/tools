//! DPAPI wrapper for `.rdp` password blobs (P2-65). Windows-only; other
//! platforms report `Unsupported` and the exporter omits the password.

use hr_core::{AppError, AppResult};

#[cfg(windows)]
mod ffi {
    use core::ffi::c_void;

    #[repr(C)]
    pub struct DataBlob {
        pub cb_data: u32,
        pub pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        pub fn CryptProtectData(
            data_in: *const DataBlob,
            description: *const u16,
            entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        pub fn CryptUnprotectData(
            data_in: *const DataBlob,
            description: *mut *mut u16,
            entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
}

#[cfg(windows)]
pub fn protect_local_machine(data: &[u8]) -> AppResult<Vec<u8>> {
    const CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x4;
    use ffi::{CryptProtectData, DataBlob, LocalFree};
    let mut input = data.to_vec();
    let input_blob = DataBlob {
        cb_data: input.len() as u32,
        pb_data: input.as_mut_ptr(),
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    // SAFETY: pointers reference live buffers for the duration of the call.
    let success = unsafe {
        CryptProtectData(
            &input_blob,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_LOCAL_MACHINE,
            &mut output,
        )
    };
    if success == 0 {
        return Err(AppError::platform("DPAPI encryption failed"));
    }
    let result = unsafe {
        let bytes = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec();
        LocalFree(output.pb_data.cast());
        bytes
    };
    Ok(result)
}

#[cfg(windows)]
pub fn unprotect_local_machine(data: &[u8]) -> AppResult<Vec<u8>> {
    use ffi::{CryptUnprotectData, DataBlob, LocalFree};
    let mut input = data.to_vec();
    let input_blob = DataBlob {
        cb_data: input.len() as u32,
        pb_data: input.as_mut_ptr(),
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    let success = unsafe {
        CryptUnprotectData(
            &input_blob,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut output,
        )
    };
    if success == 0 {
        return Err(AppError::platform("DPAPI decryption failed"));
    }
    let result = unsafe {
        let bytes = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec();
        LocalFree(output.pb_data.cast());
        bytes
    };
    Ok(result)
}

#[cfg(not(windows))]
pub fn protect_local_machine(_data: &[u8]) -> AppResult<Vec<u8>> {
    Err(AppError::unsupported(
        "DPAPI password protection is only available on Windows",
    ))
}

#[cfg(not(windows))]
pub fn unprotect_local_machine(_data: &[u8]) -> AppResult<Vec<u8>> {
    Err(AppError::unsupported(
        "DPAPI password protection is only available on Windows",
    ))
}

/// Encodes a password the way `mstsc` expects in `password 51:b:`.
pub fn rdp_password_blob(password: &str) -> AppResult<Vec<u8>> {
    let utf16: Vec<u8> = password.encode_utf16().flat_map(u16::to_le_bytes).collect();
    protect_local_machine(&utf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let blob = protect_local_machine(b"secret").unwrap();
        assert_ne!(blob, b"secret");
        assert_eq!(unprotect_local_machine(&blob).unwrap(), b"secret");
    }

    #[cfg(windows)]
    #[test]
    fn rdp_blob_contains_utf16_password_when_unprotected() {
        let blob = rdp_password_blob("hunter2").unwrap();
        let plain = unprotect_local_machine(&blob).unwrap();
        assert_eq!(
            plain,
            "hunter2"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_reports_clearly() {
        assert!(protect_local_machine(b"x").is_err());
        assert!(rdp_password_blob("x").is_err());
    }
}
