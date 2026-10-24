#![windows_subsystem = "windows"]
mod checks;
mod images;
mod settings;
mod shell;
mod text;
mod ui;
use recycle_desk::updater;

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "--wait-for-pid") {
        let pid = args
            .get(2)
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok());
        if !pid.is_some_and(|pid| updater::wait_for_parent(pid).is_ok()) {
            std::process::exit(1);
        }
    }
    if args.get(1).is_some_and(|a| a == "--self-test") {
        let Some(dir) = args.get(2) else {
            std::process::exit(2);
        };
        std::process::exit(if checks::run(std::path::Path::new(dir)) {
            0
        } else {
            1
        });
    }
    if let Err(error) = ui::run() {
        let message: Vec<u16> = format!("Recycle Desk\n{error}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                windows::core::PCWSTR(message.as_ptr()),
                windows::core::w!("Recycle Desk"),
                windows::Win32::UI::WindowsAndMessaging::MB_OK
                    | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}
