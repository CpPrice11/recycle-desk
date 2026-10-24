use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::Com::*,
        UI::Shell::{Common::*, *},
    },
};

#[derive(Clone, Debug)]
pub struct Item {
    pub id: Vec<u8>,
    pub name: String,
    pub from: String,
    pub size: Option<u64>,
    pub deleted: Option<FILETIME>,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Summary {
    pub count: u64,
    pub bytes: u64,
}

pub struct Com;
impl Com {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct Pidl(*mut ITEMIDLIST);
impl Drop for Pidl {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.cast()));
        }
    }
}

unsafe fn string(value: PWSTR) -> String {
    let s = value.to_string().unwrap_or_default();
    CoTaskMemFree(Some(value.0.cast()));
    s
}

fn key(id: u32) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
        pid: id,
    }
}

pub fn summary() -> Result<Summary> {
    unsafe {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        SHQueryRecycleBinW(PCWSTR::null(), &mut info)?;
        Ok(Summary {
            count: info.i64NumItems.max(0) as u64,
            bytes: info.i64Size.max(0) as u64,
        })
    }
}

pub fn scan() -> Result<Vec<Item>> {
    let _com = Com::new()?;
    unsafe {
        let context = |stage: &str, e: Error| Error::new(e.code(), format!("{stage}: {e}"));
        let root = Pidl(
            SHGetKnownFolderIDList(&FOLDERID_RecycleBinFolder, 0, None)
                .map_err(|e| context("Recycle bin folder", e))?,
        );
        let desktop = SHGetDesktopFolder().map_err(|e| context("Desktop Shell", e))?;
        let folder: IShellFolder = SHBindToObject(&desktop, root.0, None::<&IBindCtx>)
            .map_err(|e| context("Bind recycle bin", e))?;
        let mut enumerator = None;
        folder
            .EnumObjects(
                HWND::default(),
                (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32,
                &mut enumerator,
            )
            .ok()
            .map_err(|e| context("Enumerate recycle bin", e))?;
        let Some(enumerator) = enumerator else {
            return Ok(Vec::new());
        };
        let mut items = Vec::new();
        loop {
            let mut child = [std::ptr::null_mut()];
            let mut fetched = 0;
            let status = enumerator.Next(&mut child, Some(&mut fetched));
            status.ok().map_err(|e| context("Next recycle item", e))?;
            if fetched == 0 {
                break;
            }
            let child = Pidl(child[0]);
            let absolute = Pidl(ILCombine(Some(root.0), Some(child.0)));
            if absolute.0.is_null() {
                return Err(Error::from_hresult(E_OUTOFMEMORY));
            }
            let item: IShellItem2 = match SHCreateItemFromIDList(absolute.0) {
                Ok(i) => i,
                Err(_) => continue,
            };
            let size_key = PROPERTYKEY {
                fmtid: GUID::from_u128(0xb725f130_47ef_101a_a5f1_02608c9eebac),
                pid: 12,
            };
            let display = item
                .GetDisplayName(SIGDN_NORMALDISPLAY)
                .map(|p| string(p))
                .unwrap_or_default();
            // Recycle-bin display names may be full original paths on some
            // Windows builds. Keep the leaf name in the Name column.
            let name = std::path::Path::new(&display)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(display);
            let from = item
                .GetString(&key(2))
                .map(|p| string(p))
                .unwrap_or_default();
            let size = item.GetUInt64(&size_key).ok();
            let deleted = item.GetFileTime(&key(3)).ok();
            let id = std::slice::from_raw_parts(
                absolute.0.cast::<u8>(),
                ILGetSize(Some(absolute.0)) as usize,
            )
            .to_vec();
            items.push(Item {
                id,
                name,
                from,
                size,
                deleted,
            });
        }
        Ok(items)
    }
}

/// Resolve the recycled object itself, never its possibly reused original path.
/// Windows supplies thumbnails where available and file-type icons otherwise.
pub fn preview(id: &[u8], side: i32) -> Result<Vec<u32>> {
    if !(16..=96).contains(&side) || id.len() < 2 {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    let _com = Com::new()?;
    unsafe {
        let item: IShellItem = SHCreateItemFromIDList(id.as_ptr().cast())?;
        let factory = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .ok()
            .and_then(|path| {
                let factory = SHCreateItemFromParsingName::<_, _, IShellItemImageFactory>(
                    path,
                    None::<&IBindCtx>,
                )
                .ok();
                CoTaskMemFree(Some(path.0.cast()));
                factory
            })
            .or_else(|| item.cast::<IShellItemImageFactory>().ok())
            .ok_or_else(|| Error::from_hresult(E_NOINTERFACE))?;
        let size = SIZE { cx: side, cy: side };
        let flags = SIIGBF_RESIZETOFIT | SIIGBF_SCALEUP;
        let image = crate::images::Bitmap(
            factory
                .GetImage(size, flags)
                .or_else(|_| factory.GetImage(size, flags | SIIGBF_ICONONLY))?,
        );
        crate::images::pixels(&image, side)
    }
}

#[derive(Clone, Copy)]
pub enum Action {
    Restore,
    Delete,
}

pub fn act(hwnd: usize, ids: &[Vec<u8>], action: Action) -> Result<bool> {
    let _com = Com::new()?;
    let owner = HWND(hwnd as *mut _);
    // Only act on identities still present in the recycle bin. Display names and
    // original paths never grant permission to delete arbitrary filesystem items.
    let requested: std::collections::HashSet<&[u8]> = ids.iter().map(Vec::as_slice).collect();
    let current = scan()?;
    let selected: Vec<_> = current
        .iter()
        .filter(|i| requested.contains(i.id.as_slice()))
        .collect();
    if selected.len() != ids.len() {
        return Ok(false);
    }
    unsafe {
        match action {
            Action::Delete => {
                let operation: IFileOperation =
                    CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
                operation.SetOwnerWindow(owner)?;
                operation.SetOperationFlags(FOF_NOCONFIRMATION | FOFX_EARLYFAILURE)?;
                for entry in &selected {
                    let item: IShellItem = SHCreateItemFromIDList(entry.id.as_ptr().cast())?;
                    operation.DeleteItem(&item, None::<&IFileOperationProgressSink>)?;
                }
                operation.PerformOperations()?;
                if operation.GetAnyOperationsAborted()?.as_bool() {
                    return Ok(false);
                }
            }
            Action::Restore => {
                // Use the Shell's canonical restore verb: handles original names,
                // missing directories and collisions through the Windows UI.
                let root = Pidl(SHGetKnownFolderIDList(&FOLDERID_RecycleBinFolder, 0, None)?);
                let desktop = SHGetDesktopFolder()?;
                let folder: IShellFolder = SHBindToObject(&desktop, root.0, None::<&IBindCtx>)?;
                let children: Vec<*const ITEMIDLIST> = selected
                    .iter()
                    .map(|entry| ILFindLastID(entry.id.as_ptr().cast()).cast_const())
                    .collect();
                let menu: IContextMenu = folder.GetUIObjectOf(owner, &children, None)?;
                let popup = windows::Win32::UI::WindowsAndMessaging::CreatePopupMenu()?;
                let query = menu.QueryContextMenu(popup, 0, 1, 0x7fff, CMF_NORMAL);
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(popup);
                query.ok()?;
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd: owner,
                    lpVerb: s!("undelete"),
                    nShow: 1,
                    ..Default::default()
                };
                menu.InvokeCommand(&info)?;
            }
        }
    }
    let after = scan()?;
    Ok(!after.iter().any(|i| requested.contains(i.id.as_slice())))
}

pub fn empty(hwnd: usize) -> Result<()> {
    let _com = Com::new()?;
    unsafe {
        SHEmptyRecycleBinW(
            Some(HWND(hwnd as *mut _)),
            PCWSTR::null(),
            SHERB_NOCONFIRMATION,
        )
    }
}

pub fn date(lang: crate::text::Lang, time: FILETIME) -> String {
    use windows::Win32::{
        Globalization::*, Storage::FileSystem::FileTimeToLocalFileTime, System::Time::*,
    };
    unsafe {
        let mut local = FILETIME::default();
        let mut system = SYSTEMTIME::default();
        if FileTimeToLocalFileTime(&time, &mut local).is_err()
            || FileTimeToSystemTime(&local, &mut system).is_err()
        {
            return String::new();
        }
        let locale = if lang == crate::text::Lang::Uk {
            w!("uk-UA")
        } else {
            w!("en-GB")
        };
        let mut buf = [0u16; 80];
        let n = GetDateFormatEx(
            locale,
            DATE_SHORTDATE,
            Some(&system),
            None,
            Some(&mut buf),
            None,
        );
        if n <= 0 {
            return String::new();
        }
        let date = String::from_utf16_lossy(&buf[..n as usize - 1]);
        format!("{date} {:02}:{:02}", system.wHour, system.wMinute)
    }
}
