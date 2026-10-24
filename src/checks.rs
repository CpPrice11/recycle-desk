//! Opt-in live integration check. Mutates only freshly created, uniquely named
//! fixtures in the supplied directory; never empties the user's recycle bin.
use crate::shell::{self, Action};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    core::*,
    Win32::{Foundation::*, System::Com::*, UI::Shell::*},
};

pub fn run(dir: &Path) -> bool {
    let mut log = String::new();
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(dir.join("checkpoint.txt"), "");
    let checkpoint = |message: &str| {
        use std::io::Write;
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("checkpoint.txt"))
        {
            let _ = writeln!(file, "{message}");
        }
    };
    let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(dir)?;
        let dir = dir.canonicalize()?;
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        checkpoint("COM initialize");
        let _com = shell::Com::new()?;
        checkpoint("Get recycle folder PIDL");
        unsafe {
            let pidl = SHGetKnownFolderIDList(&FOLDERID_RecycleBinFolder, 0, None)?;
            CoTaskMemFree(Some(pidl.cast()));
        }
        checkpoint("initial Shell scan");
        let before = shell::scan()?;
        checkpoint("initial scan passed; creating fixture");
        let fixture = dir.join(format!("RecycleDesk-check-{nonce}-Україна.txt"));
        let content = b"Recycle Desk disposable integration fixture.\n";
        fs::write(&fixture, content)?;
        checkpoint("recycling fixture");
        recycle(&fixture)?;
        assert!(!fixture.exists(), "Fixture was not recycled");
        checkpoint("scanning recycled fixture");
        let items = shell::scan()?;
        let item = items
            .iter()
            .find(|i| {
                i.name == fixture.file_name().unwrap().to_string_lossy()
                    || i.name == fixture.file_stem().unwrap().to_string_lossy()
            })
            .ok_or("Fixture absent from Shell enumeration")?;
        assert_eq!(item.size, Some(content.len() as u64));
        assert!(
            item.from.contains("test-output"),
            "Original location not available"
        );
        assert!(item.deleted.is_some(), "Date deleted not available");
        let icon_preview = shell::preview(&item.id, 40);
        log.push_str("PASS: Unicode fixture recycled; Shell enumeration, size, original directory, deletion date.\n");
        checkpoint("restoring fixture");
        let restored = shell::act(0, &[item.id.clone()], Action::Restore)?;
        assert!(restored, "Shell restore incomplete");
        assert_eq!(fs::read(&fixture)?, content);
        assert_eq!(icon_preview?.len(), 40 * 40, "File icon preview");
        log.push_str("PASS: Restore returned exact bytes to original path.\n");
        checkpoint("recycling again");
        recycle(&fixture)?;
        let items = shell::scan()?;
        let item = items
            .iter()
            .find(|i| {
                i.name == fixture.file_name().unwrap().to_string_lossy()
                    || i.name == fixture.file_stem().unwrap().to_string_lossy()
            })
            .ok_or("Second fixture recycle absent")?;
        checkpoint("permanent selected deletion");
        assert!(
            shell::act(0, &[item.id.clone()], Action::Delete)?,
            "Permanent deletion incomplete"
        );
        assert!(!fixture.exists());
        let after = shell::scan()?;
        assert!(!after
            .iter()
            .any(|i| i.name == fixture.file_name().unwrap().to_string_lossy()
                || i.name == fixture.file_stem().unwrap().to_string_lossy()));
        // Prove a stale or foreign identity cannot delete an ordinary file.
        fs::write(&fixture, content)?;
        assert!(!shell::act(0, &[vec![0, 0]], Action::Delete)?);
        assert_eq!(fs::read(&fixture)?, content);
        fs::remove_file(&fixture)?;
        log.push_str("PASS: Selected permanent deletion; stale/foreign identity rejected.\n");
        checkpoint("recycled image thumbnail");
        preview_fixture(&dir, nonce)?;
        log.push_str("PASS: Recycled BMP thumbnail contains actual red/blue image pixels; exact original restored and test fixture removed.\n");
        let after = shell::scan()?;
        for item in before {
            assert!(
                after.iter().any(|i| i.id == item.id),
                "An unrelated bin identity changed during the test"
            );
        }
        log.push_str("PASS: All unrelated baseline recycle-bin identities preserved.\n");
        let summary = shell::summary()?;
        log.push_str(&format!(
            "PASS: SHQueryRecycleBinW returned {} items, {} bytes.\n",
            summary.count, summary.bytes
        ));
        Ok(())
    })();
    let passed = result.is_ok();
    if let Err(e) = result {
        log.push_str(&format!("FAIL: {e}\n"));
    }
    checkpoint("finished");
    // This report intentionally contains no unrelated file names or paths.
    let _ = fs::write(dir.join("integration.txt"), log);
    passed
}

fn preview_fixture(dir: &Path, nonce: u128) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let fixture = dir.join(format!("RecycleDesk-preview-{nonce}-Україна.bmp"));
    // A real 32×16 two-color bitmap: the test must recognize its pixels,
    // rather than accidentally accepting the file association's generic icon.
    let mut bmp = vec![0u8; 54 + 32 * 16 * 4];
    let length = bmp.len() as u32;
    bmp[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, length), (10, 54), (14, 40), (18, 32), (22, 16)] {
        bmp[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
    for (i, pixel) in bmp[54..].chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if i % 32 < 16 {
            &[0, 0, 255, 0]
        } else {
            &[255, 0, 0, 0]
        });
    }
    fs::write(&fixture, &bmp)?;
    recycle(&fixture)?;
    let items = shell::scan()?;
    let item = items
        .iter()
        .find(|i| {
            i.name == fixture.file_name().unwrap().to_string_lossy()
                || i.name == fixture.file_stem().unwrap().to_string_lossy()
        })
        .ok_or("Image fixture absent from recycle bin")?;
    let preview = shell::preview(&item.id, 40);
    let restored = shell::act(0, &[item.id.clone()], Action::Restore)?;
    if restored {
        let restored_bytes = fs::read(&fixture)?;
        fs::remove_file(&fixture)?;
        assert_eq!(restored_bytes, bmp, "Preview changed image bytes");
    }
    assert!(restored, "Image fixture restore failed");
    let pixels = preview?;
    let red = pixels
        .iter()
        .filter(|&&p| (p >> 16) & 255 > 220 && p & 255 < 30)
        .count();
    let blue = pixels
        .iter()
        .filter(|&&p| p & 255 > 220 && (p >> 16) & 255 < 30)
        .count();
    if red <= 100 || blue <= 100 {
        return Err(format!(
            "Expected actual image thumbnail, red={red}, blue={blue}, pixels={}",
            pixels.len()
        )
        .into());
    }
    Ok(())
}

fn recycle(path: &Path) -> Result<()> {
    let _com = shell::Com::new()?;
    unsafe {
        use std::os::windows::ffi::OsStrExt;
        // canonicalize() adds a Win32 extended prefix. Shell parsing names use
        // normal drive/UNC syntax, unlike direct filesystem APIs.
        let display = path.as_os_str().to_string_lossy();
        let display = display.strip_prefix(r"\\?\").unwrap_or(&display);
        let path: Vec<u16> = std::ffi::OsStr::new(display)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None::<&IBindCtx>)?;
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
        op.SetOperationFlags(
            FOFX_RECYCLEONDELETE
                | FOF_ALLOWUNDO
                | FOF_NOCONFIRMATION
                | FOF_SILENT
                | FOFX_EARLYFAILURE,
        )?;
        op.DeleteItem(&item, None::<&IFileOperationProgressSink>)?;
        op.PerformOperations()?;
        if op.GetAnyOperationsAborted()?.as_bool() {
            return Err(Error::from_hresult(E_ABORT));
        }
    }
    Ok(())
}
