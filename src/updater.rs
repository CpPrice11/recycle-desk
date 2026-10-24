//! Signed, staged GitHub updates. The UI decides when it is safe to install.
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::PathBuf,
    time::Duration,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const INTERVAL_MS: u32 = 6 * 60 * 60 * 1000;
const MAX_ARCHIVE: u64 = 32 * 1024 * 1024;
const MAX_EXE: u64 = 32 * 1024 * 1024;
const BIN: &str = "recycle-desk.exe";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Config {
    repository: String,
    key: [u8; 32],
}
impl Config {
    pub fn compiled() -> Option<Self> {
        let repository = option_env!("RECYCLE_DESK_REPOSITORY").unwrap_or("");
        if !valid_repository(repository) {
            return None;
        }
        Some(Self {
            repository: repository.into(),
            key: compiled_key()?,
        })
    }
}

fn compiled_key() -> Option<[u8; 32]> {
    let hex = option_env!("RECYCLE_DESK_VERIFYING_KEY")?;
    if hex.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    (key != [0; 32]).then_some(key)
}

/// Validate an offline release with the same pinned key and staging path.
pub fn stage_signed_archive(archive: &std::path::Path, version: &str) -> Result<Staged> {
    if !newer_stable(VERSION, version)
        || archive.file_name().and_then(|s| s.to_str()) != Some(&asset_name(version))
    {
        return Err("Invalid update version or archive name".into());
    }
    if fs::metadata(archive)?.len() > MAX_ARCHIVE {
        return Err("Update archive is too large".into());
    }
    let exe = std::env::current_exe()?;
    let parent = exe.parent().ok_or("No executable directory")?;
    let directory = tempfile::Builder::new()
        .prefix(".recycle-desk-update-")
        .tempdir_in(parent)?;
    let destination = directory.path().join(asset_name(version));
    fs::copy(archive, &destination)?;
    stage_archive(
        directory,
        destination,
        version.into(),
        compiled_key().ok_or("Missing pinned key")?,
    )
}

fn valid_repository(repo: &str) -> bool {
    let parts: Vec<_> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.len() <= 100
                && p.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        })
}

fn asset_name(version: &str) -> String {
    format!("RecycleDesk-{version}-windows-x64-update.zip")
}

fn newer_stable(current: &str, next: &str) -> bool {
    !next.contains('-')
        && !next.contains('+')
        && next.len() <= 40
        && self_update::version::bump_is_greater(current, next).unwrap_or(false)
}

pub fn can_apply(enabled: bool, visible: bool, busy: bool, modal: bool) -> bool {
    enabled && !visible && !busy && !modal
}

pub struct Staged {
    directory: tempfile::TempDir,
    version: String,
    key: [u8; 32],
}
impl Staged {
    fn reverify(self) -> Result<Self> {
        let Self {
            directory,
            version,
            key,
        } = self;
        let archive = directory.path().join(asset_name(&version));
        stage_archive(directory, archive, version, key)
    }
    /// Copies only the executable and release documentation, never settings.
    /// self-replace handles Windows' running-executable locking semantics.
    pub fn install(self) -> Result<()> {
        // A package can wait in the tray for hours. Verify again and recreate
        // the extracted files so stale or altered staging files are not used.
        let verified = self.reverify()?;
        let exe = std::env::current_exe()?;
        let parent = exe.parent().ok_or("No executable directory")?;
        for name in ["LICENSE", "README.md", "THIRD_PARTY_NOTICES.txt"] {
            fs::copy(verified.directory.path().join(name), parent.join(name))?;
        }
        self_replace::self_replace(verified.directory.path().join(BIN))?;
        Ok(())
    }
}

pub fn check(config: &Config) -> Result<Option<Staged>> {
    let (owner, repo) = config
        .repository
        .split_once('/')
        .ok_or("Invalid release repository")?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .https_only(true)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .into();
    let updater = self_update::backends::github::Update::configure()
        .repo_owner(owner)
        .repo_name(repo)
        .bin_name(BIN)
        .current_version(VERSION)
        .ureq_agent(agent.clone())
        .no_confirm(true)
        .show_output(false)
        .timeout(Duration::from_secs(20))
        .build()?;
    let releases = updater.get_latest_release()?;
    let Some(release) = releases.latest() else {
        return Ok(None);
    };
    if !newer_stable(VERSION, release.version()) {
        return Ok(None);
    }
    let filename = asset_name(release.version());
    let asset = release
        .assets()
        .iter()
        .find(|a| a.name() == filename)
        .ok_or("Signed update asset is missing")?;
    // Accept only this repository's GitHub release downloads. No user-supplied
    // mirrors or arbitrary URLs, even if an API response is malformed.
    let prefix = format!(
        "https://github.com/{}/releases/download/",
        config.repository
    );
    if !asset.download_url().starts_with(&prefix) {
        return Err("Unexpected update download origin".into());
    }
    let exe = std::env::current_exe()?;
    let parent = exe.parent().ok_or("No executable directory")?;
    let directory = tempfile::Builder::new()
        .prefix(".recycle-desk-update-")
        .tempdir_in(parent)?;
    // The filename is part of the Ed25519 signature context: it binds the
    // version and platform, and prevents renaming an old signed package.
    let archive = directory.path().join(&filename);
    let mut output = File::create(&archive)?;
    let mut response = agent
        .get(asset.download_url())
        .header("User-Agent", "RecycleDesk-Updater")
        .call()?;
    let size = io::copy(
        &mut response.body_mut().as_reader().take(MAX_ARCHIVE + 1),
        &mut output,
    )?;
    if size > MAX_ARCHIVE {
        return Err("Update archive is too large".into());
    }
    output.sync_all()?;
    drop(output);
    stage_archive(directory, archive, release.version().to_owned(), config.key).map(Some)
}

fn stage_archive(
    directory: tempfile::TempDir,
    archive: PathBuf,
    version: String,
    key: [u8; 32],
) -> Result<Staged> {
    if key == [0; 32] {
        return Err("Missing update verification key".into());
    }
    if fs::metadata(&archive)?.len() > MAX_ARCHIVE {
        return Err("Update archive is too large".into());
    }
    self_update::verify_signature(&archive, &[key])?;
    let mut zip = zip::ZipArchive::new(File::open(&archive)?)?;
    for name in [BIN, "LICENSE", "README.md", "THIRD_PARTY_NOTICES.txt"] {
        let mut entry = zip.by_name(name)?;
        let limit = match name {
            BIN => MAX_EXE,
            "THIRD_PARTY_NOTICES.txt" => 4 * 1024 * 1024,
            _ => 1024 * 1024,
        };
        if entry.is_dir() || entry.is_symlink() || entry.size() > limit {
            return Err("Invalid update entry".into());
        }
        let mut output = File::create(directory.path().join(name))?;
        let size = io::copy(&mut entry.by_ref().take(limit + 1), &mut output)?;
        if size > limit {
            return Err("Expanded update entry is too large".into());
        }
        output.flush()?;
        output.sync_all()?;
    }
    // Fail before replacement for accidentally uploaded non-Windows binaries.
    let mut header = [0u8; 2];
    File::open(directory.path().join(BIN))?.read_exact(&mut header)?;
    if &header != b"MZ" {
        return Err("Update is not a Windows executable".into());
    }
    Ok(Staged {
        directory,
        version,
        key,
    })
}

/// New process waits until the old one has exited and released its mutex.
pub fn wait_for_parent(pid: u32) -> Result<()> {
    use windows::Win32::{Foundation::*, System::Threading::*};
    unsafe {
        let process = match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(p) => p,
            Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => {
                return Ok(())
            }
            Err(e) => return Err(e.into()),
        };
        let result = WaitForSingleObject(process, 60000);
        let _ = CloseHandle(process);
        if result != WAIT_OBJECT_0 {
            return Err("Previous process has not exited".into());
        }
    }
    Ok(())
}

pub fn relaunch(arguments: &[&std::ffi::OsStr]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(std::env::current_exe()?)
        .args(["--wait-for-pid", &std::process::id().to_string()])
        .args(arguments)
        .creation_flags(0x08000000)
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Seek, SeekFrom};
    use std::path::Path;
    fn archive(path: &Path, key: &zipsign_api::SigningKey, signed: bool) -> Result<()> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in [
            (BIN, &b"MZfixture"[..]),
            ("README.md", &b"readme"[..]),
            ("LICENSE", &b"MIT"[..]),
            ("THIRD_PARTY_NOTICES.txt", &b"Third party licenses"[..]),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())?;
            zip.write_all(data)?;
        }
        let mut data = zip.finish()?;
        data.seek(SeekFrom::Start(0))?;
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path)?;
        if signed {
            let context = path.file_name().unwrap().to_str().unwrap().as_bytes();
            zipsign_api::sign::copy_and_sign_zip(
                &mut data,
                &mut output,
                &[key.clone()],
                Some(context),
            )?;
        } else {
            io::copy(&mut data, &mut output)?;
        }
        Ok(())
    }
    #[test]
    fn signed_staging_rejects_unsigned_wrong_key_and_renaming() -> Result<()> {
        let key = zipsign_api::SigningKey::from_bytes(&[42; 32]);
        for mode in ["valid", "unsigned", "wrong-key", "renamed", "tampered"] {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join(asset_name("0.2.0"));
            archive(&path, &key, mode != "unsigned")?;
            let path = if mode == "renamed" {
                let new = dir.path().join(asset_name("0.3.0"));
                fs::rename(&path, &new)?;
                new
            } else {
                path
            };
            if mode == "tampered" {
                let mut bytes = fs::read(&path)?;
                bytes[40] ^= 1;
                fs::write(&path, bytes)?;
            }
            let public = if mode == "wrong-key" {
                zipsign_api::SigningKey::from_bytes(&[43; 32])
                    .verifying_key()
                    .to_bytes()
            } else {
                key.verifying_key().to_bytes()
            };
            let result = stage_archive(dir, path, "0.2.0".into(), public);
            assert_eq!(result.is_ok(), mode == "valid", "{mode}");
            if let Ok(staged) = result {
                assert_eq!(fs::read(staged.directory.path().join(BIN))?, b"MZfixture");
            }
        }
        Ok(())
    }
    #[test]
    fn versions_repositories_and_idle_gate() {
        assert!(newer_stable("0.1.0", "0.2.0"));
        for next in ["0.1.0", "0.0.9", "0.2.0-beta.1", "../../evil", "garbage"] {
            assert!(!newer_stable("0.1.0", next));
        }
        assert!(valid_repository("owner/RecycleDesk"));
        for repo in ["", "a/b/c", "a/..", "https://github.com/a/b", "a/b?x"] {
            assert!(!valid_repository(repo));
        }
        assert!(can_apply(true, false, false, false));
        for state in [
            (false, false, false, false),
            (true, true, false, false),
            (true, false, true, false),
            (true, false, false, true),
        ] {
            assert!(!can_apply(state.0, state.1, state.2, state.3));
        }
    }
    #[test]
    fn staging_is_reverified_before_install() -> Result<()> {
        let key = zipsign_api::SigningKey::from_bytes(&[44; 32]);
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(asset_name("0.2.0"));
        archive(&path, &key, true)?;
        let staged = stage_archive(dir, path, "0.2.0".into(), key.verifying_key().to_bytes())?;
        fs::write(
            staged.directory.path().join(BIN),
            b"MZmodified while waiting",
        )?;
        let staged = staged.reverify()?;
        assert_eq!(fs::read(staged.directory.path().join(BIN))?, b"MZfixture");
        let archive = staged.directory.path().join(asset_name("0.2.0"));
        let mut bytes = fs::read(&archive)?;
        bytes[40] ^= 1;
        fs::write(archive, bytes)?;
        assert!(staged.reverify().is_err());
        Ok(())
    }
}
