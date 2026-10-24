//! Isolated test of the real signed staging, installation and relaunch code.
use recycle_desk::updater;
use std::{env, fs, path::Path};
use windows::{
    core::w,
    Win32::{Foundation::*, System::Threading::*},
};
fn main() -> updater::Result<()> {
    let args: Vec<_> = env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "--wait-for-pid") {
        updater::wait_for_parent(args.get(2).and_then(|s| s.to_str()).ok_or("PID")?.parse()?)?;
    }
    let mutex = unsafe { CreateMutexW(None, false, w!("Local\\RecycleDesk.UpdateProbe"))? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Err("Previous process still owns the mutex".into());
    }
    if args.get(1).is_some_and(|s| s == "--replace") && args.len() == 5 {
        let staged =
            updater::stage_signed_archive(Path::new(&args[2]), args[3].to_str().ok_or("Version")?)?;
        staged.install()?;
        updater::relaunch(&[std::ffi::OsStr::new("--probe"), &args[4]])?;
        std::thread::sleep(std::time::Duration::from_millis(400));
    } else if args.get(3).is_some_and(|s| s == "--probe") && args.len() == 5 {
        fs::write(
            &args[4],
            option_env!("RECYCLE_DESK_PROBE_VERSION").unwrap_or("old"),
        )?;
    } else {
        return Err("Probe arguments missing".into());
    }
    let _ = unsafe { CloseHandle(mutex) };
    Ok(())
}
