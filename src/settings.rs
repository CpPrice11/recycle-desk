use crate::text::Lang;
use std::{fs, path::PathBuf};
use windows::{core::*, Win32::System::Registry::*};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

pub struct Settings {
    pub lang: Lang,
    pub theme: Theme,
    pub portable: bool,
    pub path: PathBuf,
    pub auto_update: bool,
}

impl Settings {
    pub fn load() -> Self {
        let dir = std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let portable = dir.join("portable.flag").exists();
        let path = if portable {
            dir.join("recycle-desk.ini")
        } else {
            PathBuf::from(
                std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| dir.clone().into_os_string()),
            )
            .join("RecycleDesk")
            .join("settings.ini")
        };
        let lang = unsafe {
            if windows::Win32::Globalization::GetUserDefaultUILanguage() & 0x3ff == 0x22 {
                Lang::Uk
            } else {
                Lang::En
            }
        };
        let mut settings = Self {
            lang,
            theme: Theme::System,
            portable,
            path,
            auto_update: true,
        };
        if let Ok(data) = fs::read_to_string(&settings.path) {
            for line in data.lines() {
                match line.trim() {
                    "language=uk" => settings.lang = Lang::Uk,
                    "language=en" => settings.lang = Lang::En,
                    "theme=dark" => settings.theme = Theme::Dark,
                    "theme=light" => settings.theme = Theme::Light,
                    "theme=system" => settings.theme = Theme::System,
                    "auto_update=false" => settings.auto_update = false,
                    "auto_update=true" => settings.auto_update = true,
                    _ => {}
                }
            }
        }
        settings
    }
    pub fn save(&self) -> std::io::Result<()> {
        let lang = if self.lang == Lang::Uk { "uk" } else { "en" };
        let theme = match self.theme {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        };
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = self.path.with_extension("tmp");
        fs::write(
            &temp,
            format!(
                "language={lang}\ntheme={theme}\nauto_update={}\n",
                self.auto_update
            ),
        )?;
        // Same-volume atomic replacement; no intermediate removal of the saved file.
        unsafe {
            use windows::Win32::Storage::FileSystem::*;
            let from: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<_> = self.path.as_os_str().encode_wide().chain(Some(0)).collect();
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .map_err(std::io::Error::other)?;
        }
        Ok(())
    }
}
use std::os::windows::ffi::OsStrExt;

const RUN_PATH: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_NAME: PCWSTR = w!("RecycleDesk");

pub fn startup_enabled() -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_PATH,
            Some(0),
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let mut size = 0;
        let found = RegQueryValueExW(key, RUN_NAME, None, None, None, Some(&mut size)).is_ok();
        let _ = RegCloseKey(key);
        found
    }
}

pub fn set_startup(enabled: bool) -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_PATH,
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let result = if enabled {
            let exe = std::env::current_exe().map_err(|_| Error::from_win32())?;
            let command = format!("\"{}\" --tray", exe.display());
            let data: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
            let data = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
            RegSetValueExW(key, RUN_NAME, Some(0), REG_SZ, Some(data)).ok()
        } else {
            let status = RegDeleteValueW(key, RUN_NAME);
            if status == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                status.ok()
            }
        };
        let _ = RegCloseKey(key);
        result
    }
}

pub fn system_dark() -> bool {
    unsafe {
        let mut value = 1u32;
        let mut size = 4;
        let _ = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        );
        value == 0
    }
}

/// Refresh Inno's version after a self-update, only for this exact install path.
pub fn refresh_installed_version() {
    if Settings::load().portable {
        return;
    }
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER,w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{DA427122-1CB5-4EE5-AC8D-58730802222C}_is1"),Some(0),KEY_QUERY_VALUE|KEY_SET_VALUE,&mut key).is_err(){return;}
        let mut location = [0u16; 32768];
        let mut size = (location.len() * 2) as u32;
        if RegQueryValueExW(
            key,
            w!("InstallLocation"),
            None,
            None,
            Some(location.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .is_ok()
        {
            let len = location
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(location.len());
            let installed =
                PathBuf::from(String::from_utf16_lossy(&location[..len])).canonicalize();
            let current = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().and_then(|p| p.canonicalize().ok()));
            if installed
                .ok()
                .as_ref()
                .is_some_and(|p| Some(p) == current.as_ref())
            {
                let value: Vec<u16> = env!("CARGO_PKG_VERSION")
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
                let bytes =
                    std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), value.len() * 2);
                let _ = RegSetValueExW(key, w!("DisplayVersion"), Some(0), REG_SZ, Some(bytes));
            }
        }
        let _ = RegCloseKey(key);
    }
}
