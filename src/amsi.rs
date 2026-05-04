use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// AMSI scan result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmsiResult {
    /// The file is clean.
    Clean,
    /// Malware detected.
    Detected,
}

/// Windows AMSI session handle wrapper.
pub struct AmsiSession {
    context: windows_sys::Win32::System::Antimalware::HAMSICONTEXT,
    session: windows_sys::Win32::System::Antimalware::HAMSISESSION,
}

impl AmsiSession {
    /// Create a new AMSI session. Returns None if AMSI is unavailable.
    pub fn new() -> Option<Self> {
        let app_name: Vec<u16> = OsStr::new("FastZIP")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut context = std::ptr::null_mut();
        let hr = unsafe {
            windows_sys::Win32::System::Antimalware::AmsiInitialize(app_name.as_ptr(), &mut context)
        };

        if hr != 0 || context.is_null() {
            return None;
        }

        let mut session = std::ptr::null_mut();
        let hr = unsafe {
            windows_sys::Win32::System::Antimalware::AmsiOpenSession(context, &mut session)
        };

        if hr != 0 {
            unsafe {
                windows_sys::Win32::System::Antimalware::AmsiUninitialize(context);
            }
            return None;
        }

        Some(AmsiSession { context, session })
    }

    /// Scan a buffer. Returns `AmsiResult::Detected` if malware is found.
    pub fn scan(&self, content_name: &str, data: &[u8]) -> AmsiResult {
        let name: Vec<u16> = OsStr::new(content_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut result = 0i32;
        let hr = unsafe {
            windows_sys::Win32::System::Antimalware::AmsiScanBuffer(
                self.context,
                data.as_ptr() as *const std::ffi::c_void,
                data.len() as u32,
                name.as_ptr(),
                self.session,
                &mut result,
            )
        };

        if hr != 0 {
            // HRESULT error — treat as undetected (fail open for usability)
            return AmsiResult::Clean;
        }

        if result == windows_sys::Win32::System::Antimalware::AMSI_RESULT_DETECTED {
            AmsiResult::Detected
        } else {
            AmsiResult::Clean
        }
    }
}

impl Drop for AmsiSession {
    fn drop(&mut self) {
        if !self.session.is_null() {
            unsafe {
                windows_sys::Win32::System::Antimalware::AmsiCloseSession(
                    self.context,
                    self.session,
                );
            }
        }
        if !self.context.is_null() {
            unsafe {
                windows_sys::Win32::System::Antimalware::AmsiUninitialize(self.context);
            }
        }
    }
}
