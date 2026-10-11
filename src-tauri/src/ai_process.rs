use crate::ai_store::Result;
use std::path::Path;
pub fn command(executable: &Path, runtime: &Path) -> Result<std::process::Command> {
    #[cfg(target_os = "macos")]
    {
        let guard = runtime.join("daxiong-ai-guard");
        if !guard.is_file() {
            return Err("模型进程保护运行器缺失，请修复安装。".into());
        }
        let mut command = std::process::Command::new(guard);
        command
            .arg(std::process::id().to_string())
            .arg("--")
            .arg(executable);
        Ok(command)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = runtime;
        Ok(std::process::Command::new(executable))
    }
}
pub fn kill(child: &mut std::process::Child) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        if unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    child.kill()
}
// Windows job closes automatically if the desktop process exits or crashes.
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn job_helper_abnormal_exit() {
        let Ok(path) = std::env::var("DAXIONG_JOB_TEST_PID") else {
            return;
        };
        let mut command = std::process::Command::new("cmd.exe");
        command.args(["/C", "ping -n 60 127.0.0.1 >nul"]);
        let child = own(command.spawn().unwrap()).unwrap();
        std::fs::write(path, child.id().to_string()).unwrap();
        // Deliberately bypass Rust destructors: Windows must close the job itself.
        std::process::exit(70);
    }
    #[test]
    fn windows_job_reaps_children_on_abnormal_parent_exit() {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{OpenProcess, WaitForSingleObject},
        };
        let path = std::env::temp_dir().join(format!("daxiong-job-{}.pid", uuid::Uuid::new_v4()));
        let mut parent = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ai_process::tests::job_helper_abnormal_exit",
                "--nocapture",
            ])
            .env("DAXIONG_JOB_TEST_PID", &path)
            .spawn()
            .unwrap();
        let started = std::time::Instant::now();
        while !path.exists() && started.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(path.exists(), "job child did not start");
        let pid: u32 = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        // Acquire the process before waiting for the helper; if already reaped, it is gone.
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
        assert_eq!(parent.wait().unwrap().code(), Some(70));
        if !handle.is_null() {
            let status = unsafe { WaitForSingleObject(handle, 5000) };
            unsafe {
                CloseHandle(handle);
            }
            assert_eq!(status, 0, "model child survived desktop exit");
        }
        let _ = std::fs::remove_file(path);
    }
}
