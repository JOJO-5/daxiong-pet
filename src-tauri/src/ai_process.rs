//! Windows job closes automatically if the desktop process exits or crashes.
use std::process::Child;
pub fn own(mut child: Child) -> std::io::Result<Child> {
    #[cfg(windows)]
    {
        use std::{os::windows::io::AsRawHandle, sync::OnceLock};
        use windows_sys::Win32::{
            Foundation::HANDLE,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
        };
        static JOB: OnceLock<std::result::Result<usize, ()>> = OnceLock::new();
        let job = JOB.get_or_init(|| unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return Err(());
            }
            Ok(handle as usize)
        });
        if job.as_ref().is_err()
            || unsafe {
                AssignProcessToJobObject(
                    *job.as_ref().unwrap() as HANDLE,
                    child.as_raw_handle() as HANDLE,
                )
            } == 0
        {
            let error = std::io::Error::last_os_error();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }
    #[cfg(not(windows))]
    let _ = &mut child;
    Ok(child)
}
