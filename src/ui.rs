use crate::{
    settings::{self, Settings, Theme},
    shell::{self, Action, Item, Summary},
    text::{self, Key, Lang},
};
use recycle_desk::updater;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    mem::size_of,
    sync::mpsc::{self, Receiver, Sender},
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{LibraryLoader::*, Threading::*},
        UI::{
            Accessibility::*, Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

const CLASS: PCWSTR = w!("RecycleDesk.Main.v1");
const TRAY: u32 = WM_APP + 1;
const RESULT: u32 = WM_APP + 2;
const CHANGE: u32 = WM_APP + 3;
const OPEN: u32 = WM_APP + 4;
const APPLY_UPDATE: u32 = WM_APP + 5;
const TRAY_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const REFRESH: usize = 101;
const RESTORE: usize = 102;
const DELETE: usize = 103;
const EMPTY: usize = 104;
const EXPLORER: usize = 105;
const SETTINGS: usize = 106;
const EXIT: usize = 107;
const SEARCH: usize = 108;
const LANG_UK: usize = 201;
const LANG_EN: usize = 202;
const THEME_SYSTEM: usize = 203;
const THEME_LIGHT: usize = 204;
const THEME_DARK: usize = 205;
const STARTUP: usize = 206;
const AUTO_UPDATE: usize = 207;

enum Reply {
    Update(std::result::Result<Option<updater::Staged>, String>),
    List(std::result::Result<Vec<Item>, String>),
    Stats(std::result::Result<Summary, String>),
    Operation(std::result::Result<bool, String>),
}
enum Effect {
    None,
    Command(usize),
    Error(String),
    InstallUpdate,
}

#[derive(Clone, Copy)]
struct Palette {
    base: COLORREF,
    surface: COLORREF,
    text: COLORREF,
    muted: COLORREF,
    accent: COLORREF,
    on_accent: COLORREF,
    danger: COLORREF,
}
#[derive(Clone, Copy)]
struct Paint {
    palette: Palette,
    brush: HBRUSH,
    font: HFONT,
    header: HWND,
    dpi: i32,
}
// Painting is reentrant: WM_SETTEXT/MoveWindow can synchronously ask the parent
// for colors while App is mutably borrowed. Keep this tiny copy independently.
struct WindowState {
    app: RefCell<App>,
    paint: Cell<Paint>,
    modal_depth: Cell<u32>,
}
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn set_text(hwnd: HWND, text: &str) {
    let _ = SetWindowTextW(hwnd, PCWSTR(wide(text).as_ptr()));
}

struct App {
    hwnd: HWND,
    settings: Settings,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
    items: Vec<Item>,
    visible: Vec<usize>,
    summary: Summary,
    stats_ok: bool,
    sort: usize,
    descending: bool,
    busy: bool,
    pending: bool,
    view_error: bool,
    last_status: Option<Key>,
    controls: Vec<HWND>,
    list: HWND,
    search: HWND,
    status: HWND,
    total: HWND,
    font: HFONT,
    title_font: HFONT,
    brush: HBRUSH,
    palette: Palette,
    dark: bool,
    icon: HICON,
    watch: u32,
    taskbar_created: u32,
    dpi: i32,
    update_busy: bool,
    staged: Option<updater::Staged>,
    update_status: Key,
    shutting_down: bool,
    opening_window: bool,
}

impl App {
    fn new(tx: Sender<Reply>, rx: Receiver<Reply>) -> Self {
        Self {
            hwnd: HWND::default(),
            settings: Settings::load(),
            tx,
            rx,
            items: vec![],
            visible: vec![],
            summary: Summary::default(),
            stats_ok: false,
            sort: 3,
            descending: true,
            busy: false,
            pending: false,
            view_error: false,
            last_status: None,
            controls: vec![],
            list: HWND::default(),
            search: HWND::default(),
            status: HWND::default(),
            total: HWND::default(),
            font: HFONT::default(),
            title_font: HFONT::default(),
            brush: HBRUSH::default(),
            palette: Palette {
                base: rgb(30, 30, 46),
                surface: rgb(49, 50, 68),
                text: rgb(205, 214, 244),
                muted: rgb(166, 173, 200),
                accent: rgb(137, 180, 250),
                on_accent: rgb(30, 30, 46),
                danger: rgb(243, 139, 168),
            },
            dark: true,
            icon: HICON::default(),
            watch: 0,
            taskbar_created: 0,
            dpi: 96,
            update_busy: false,
            staged: None,
            update_status: Key::UpdateUnconfigured,
            shutting_down: false,
            opening_window: !std::env::args().any(|a| a == "--tray"),
        }
    }
    fn t(&self, key: Key) -> &'static str {
        text::tr(self.settings.lang, key)
    }
    fn px(&self, n: i32) -> i32 {
        n * self.dpi / 96
    }
    unsafe fn control(
        &mut self,
        class: PCWSTR,
        label: &str,
        style: WINDOW_STYLE,
        id: usize,
    ) -> Result<HWND> {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(wide(label).as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            Some(self.hwnd),
            Some(HMENU(id as *mut _)),
            None,
            None,
        )?;
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(self.font.0 as usize)),
            Some(LPARAM(1)),
        );
        self.controls.push(hwnd);
        Ok(hwnd)
    }
    unsafe fn theme(&mut self) {
        self.dark = match self.settings.theme {
            Theme::System => settings::system_dark(),
            Theme::Dark => true,
            Theme::Light => false,
        };
        self.palette = if self.dark {
            Palette {
                base: rgb(30, 30, 46),
                surface: rgb(49, 50, 68),
                text: rgb(205, 214, 244),
                muted: rgb(166, 173, 200),
                accent: rgb(137, 180, 250),
                on_accent: rgb(30, 30, 46),
                danger: rgb(243, 139, 168),
            }
        } else {
            Palette {
                base: rgb(239, 241, 245),
                surface: rgb(220, 224, 232),
                text: rgb(76, 79, 105),
                muted: rgb(92, 95, 119),
                accent: rgb(30, 102, 245),
                on_accent: rgb(239, 241, 245),
                danger: rgb(170, 20, 52),
            }
        };
        let mut hc = HIGHCONTRASTW {
            cbSize: size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        let _ = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            Some((&mut hc as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        );
        if hc.dwFlags.0 & HCF_HIGHCONTRASTON.0 != 0 {
            self.palette = Palette {
                base: COLORREF(GetSysColor(COLOR_WINDOW)),
                surface: COLORREF(GetSysColor(COLOR_BTNFACE)),
                text: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                muted: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                accent: COLORREF(GetSysColor(COLOR_HIGHLIGHT)),
                on_accent: COLORREF(GetSysColor(COLOR_HIGHLIGHTTEXT)),
                danger: COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
            };
        }
        if !self.brush.is_invalid() {
            let _ = DeleteObject(self.brush.into());
        }
        self.brush = CreateSolidBrush(self.palette.base);
        self.publish_paint();
        let dark = BOOL::from(self.dark);
        let _ = DwmSetWindowAttribute(
            self.hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const BOOL).cast(),
            size_of::<BOOL>() as u32,
        );
        if !self.list.is_invalid() {
            let _ = SetWindowTheme(
                self.list,
                if self.dark {
                    w!("DarkMode_Explorer")
                } else {
                    w!("Explorer")
                },
                None,
            );
            SendMessageW(
                self.list,
                LVM_SETBKCOLOR,
                None,
                Some(LPARAM(self.palette.base.0 as isize)),
            );
            SendMessageW(
                self.list,
                LVM_SETTEXTBKCOLOR,
                None,
                Some(LPARAM(self.palette.base.0 as isize)),
            );
            SendMessageW(
                self.list,
                LVM_SETTEXTCOLOR,
                None,
                Some(LPARAM(self.palette.text.0 as isize)),
            );
        }
        let _ = InvalidateRect(Some(self.hwnd), None, true);
        for &hwnd in &self.controls {
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
    }
    unsafe fn publish_paint(&self) {
        let header = if self.list.is_invalid() {
            HWND::default()
        } else {
            HWND(SendMessageW(self.list, LVM_GETHEADER, None, None).0 as *mut _)
        };
        window_state(self.hwnd).paint.set(Paint {
            palette: self.palette,
            brush: self.brush,
            font: self.font,
            header,
            dpi: self.dpi,
        });
    }
    unsafe fn make_controls(&mut self) -> Result<()> {
        self.dpi = GetDpiForWindow(self.hwnd) as i32;
        self.fonts();
        let title = self.control(w!("STATIC"), self.t(Key::Title), WINDOW_STYLE::default(), 0)?;
        SendMessageW(
            title,
            WM_SETFONT,
            Some(WPARAM(self.title_font.0 as usize)),
            Some(LPARAM(1)),
        );
        self.control(
            w!("STATIC"),
            self.t(Key::Subtitle),
            WINDOW_STYLE::default(),
            0,
        )?;
        self.total = self.control(w!("STATIC"), "", WINDOW_STYLE::default(), 0)?;
        self.control(
            w!("STATIC"),
            self.t(Key::Search),
            WINDOW_STYLE::default(),
            0,
        )?;
        self.search = self.control(
            w!("EDIT"),
            "",
            WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            SEARCH,
        )?;
        SendMessageW(self.search, EM_SETLIMITTEXT, Some(WPARAM(256)), None);
        for (id, key) in [
            (REFRESH, Key::Refresh),
            (RESTORE, Key::Restore),
            (DELETE, Key::Delete),
            (EMPTY, Key::Empty),
            (SETTINGS, Key::Settings),
        ] {
            self.control(
                w!("BUTTON"),
                self.t(key),
                WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
                id,
            )?;
        }
        self.list = self.control(
            WC_LISTVIEWW,
            "",
            WS_TABSTOP | WS_BORDER | WINDOW_STYLE(LVS_REPORT | LVS_OWNERDATA | LVS_SHOWSELALWAYS),
            110,
        )?;
        SendMessageW(
            self.list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            None,
            Some(LPARAM(
                (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_LABELTIP) as isize,
            )),
        );
        for (i, key) in [Key::Name, Key::From, Key::Size, Key::Date]
            .iter()
            .enumerate()
        {
            let text = wide(self.t(*key));
            let col = LVCOLUMNW {
                mask: LVCF_TEXT | LVCF_WIDTH | LVCF_FMT,
                cx: self.px([220, 300, 100, 155][i]),
                pszText: PWSTR(text.as_ptr() as *mut _),
                fmt: if i == 2 { LVCFMT_RIGHT } else { LVCFMT_LEFT },
                ..Default::default()
            };
            SendMessageW(
                self.list,
                LVM_INSERTCOLUMNW,
                Some(WPARAM(i)),
                Some(LPARAM((&col as *const LVCOLUMNW) as isize)),
            );
        }
        self.status = self.control(w!("STATIC"), self.t(Key::Ready), WINDOW_STYLE::default(), 0)?;
        self.theme();
        self.layout();
        self.update_summary();
        self.scan();
        Ok(())
    }
    unsafe fn fonts(&mut self) {
        if !self.font.is_invalid() {
            let _ = DeleteObject(self.font.into());
        }
        if !self.title_font.is_invalid() {
            let _ = DeleteObject(self.title_font.into());
        }
        self.font = CreateFontW(
            -self.px(14),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        );
        self.title_font = CreateFontW(
            -self.px(28),
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        );
        self.publish_paint();
        for &hwnd in &self.controls {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        if let Some(&title) = self.controls.first() {
            SendMessageW(
                title,
                WM_SETFONT,
                Some(WPARAM(self.title_font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        self.publish_paint();
    }
    unsafe fn layout(&self) {
        if self.controls.is_empty() {
            return;
        }
        let mut r = RECT::default();
        let _ = GetClientRect(self.hwnd, &mut r);
        let w = r.right * 96 / self.dpi;
        let h = r.bottom * 96 / self.dpi;
        let place = |hwnd, x, y, width, height| {
            let _ = MoveWindow(
                hwnd,
                self.px(x),
                self.px(y),
                self.px(width),
                self.px(height),
                true,
            );
        };
        place(self.controls[0], 24, 16, w - 348, 40);
        place(self.controls[1], 24, 62, w - 48, 24);
        place(self.total, w - 300, 28, 276, 24);
        place(self.controls[3], 24, 104, w - 48, 20);
        place(self.search, 24, 128, w - 48, 32);
        for (i, &hwnd) in self.controls[5..10].iter().enumerate() {
            let bw = (w - 48 - 4 * 8) / 5;
            place(hwnd, 24 + i as i32 * (bw + 8), 176, bw, 36);
        }
        place(self.list, 24, 228, w - 48, (h - 278).max(80));
        place(self.status, 24, h - 34, w - 48, 24);
        self.fit_columns();
    }
    unsafe fn fit_columns(&self) {
        if self.list.is_invalid() {
            return;
        }
        let mut rect = RECT::default();
        let _ = GetClientRect(self.list, &mut rect);
        let remaining = (rect.right - rect.left - self.px(260) - 2).max(self.px(200));
        let name = remaining * 45 / 100;
        for (i, width) in [name, remaining - name, self.px(100), self.px(160)]
            .iter()
            .enumerate()
        {
            SendMessageW(
                self.list,
                LVM_SETCOLUMNWIDTH,
                Some(WPARAM(i)),
                Some(LPARAM(*width as isize)),
            );
        }
    }
    unsafe fn show(&mut self) -> Result<()> {
        if self.controls.is_empty() {
            self.make_controls()?;
        } else {
            self.scan();
        }
        let _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(self.hwnd);
        let _ = SetFocus(Some(self.search));
        Ok(())
    }
    unsafe fn hide(&mut self) {
        let _ = ShowWindow(self.hwnd, SW_HIDE);
        for hwnd in self.controls.drain(..) {
            let _ = DestroyWindow(hwnd);
        }
        self.list = HWND::default();
        self.search = HWND::default();
        self.status = HWND::default();
        self.total = HWND::default();
        self.items = Vec::new();
        self.visible = Vec::new();
        self.publish_paint();
    }
    fn send_work(&self, work: impl FnOnce() -> Reply + Send + 'static) {
        let tx = self.tx.clone();
        let hwnd = self.hwnd.0 as usize;
        std::thread::spawn(move || {
            let reply = work();
            if tx.send(reply).is_ok() {
                unsafe {
                    let _ = PostMessageW(Some(HWND(hwnd as *mut _)), RESULT, WPARAM(0), LPARAM(0));
                }
            }
        });
    }
    unsafe fn scan(&mut self) {
        if self.busy {
            self.pending = true;
            return;
        }
        if self.controls.is_empty() {
            self.stats();
            return;
        }
        self.busy = true;
        self.buttons();
        set_text(self.status, self.t(Key::Loading));
        self.send_work(|| Reply::List(shell::scan().map_err(|e| e.to_string())));
    }
    fn stats(&self) {
        self.send_work(|| Reply::Stats(shell::summary().map_err(|e| e.to_string())));
    }
    fn check_updates(&mut self) {
        if !self.settings.auto_update {
            self.update_status = Key::UpdateDisabled;
            return;
        }
        let Some(config) = updater::Config::compiled() else {
            self.update_status = Key::UpdateUnconfigured;
            return;
        };
        if self.update_busy || self.staged.is_some() {
            return;
        }
        self.update_busy = true;
        self.update_status = Key::UpdateChecking;
        self.send_work(move || Reply::Update(updater::check(&config).map_err(|e| e.to_string())));
    }
    unsafe fn buttons(&self) {
        let n = if self.list.is_invalid() {
            0
        } else {
            SendMessageW(self.list, LVM_GETSELECTEDCOUNT, None, None).0
        };
        if n > 0 && !self.busy && !self.status.is_invalid() {
            set_text(
                self.status,
                &format!(
                    "{}: {}",
                    self.t(Key::Selection),
                    text::count(self.settings.lang, n as u64)
                ),
            );
        }
        for id in [REFRESH, RESTORE, DELETE, EMPTY] {
            if let Ok(button) = GetDlgItem(Some(self.hwnd), id as i32) {
                let enabled = !self.busy
                    && match id {
                        RESTORE | DELETE => n > 0,
                        EMPTY => self.stats_ok && self.summary.count > 0,
                        _ => true,
                    };
                let _ = EnableWindow(button, enabled);
            }
        }
    }
    unsafe fn update_summary(&self) {
        let label = if self.stats_ok {
            format!(
                "{}  ·  {}",
                text::count(self.settings.lang, self.summary.count),
                text::bytes(self.settings.lang, self.summary.bytes)
            )
        } else {
            self.t(Key::Loading).to_string()
        };
        if !self.total.is_invalid() {
            set_text(self.total, &label);
        }
        self.buttons();
    }
    unsafe fn selected(&self) -> Vec<Vec<u8>> {
        if self.list.is_invalid() {
            return vec![];
        }
        let mut ids = vec![];
        let mut index = -1isize;
        loop {
            index = SendMessageW(
                self.list,
                LVM_GETNEXTITEM,
                Some(WPARAM(index as usize)),
                Some(LPARAM(LVNI_SELECTED as isize)),
            )
            .0;
            if index < 0 {
                break;
            }
            if let Some(&item) = self.visible.get(index as usize) {
                ids.push(self.items[item].id.clone());
            }
        }
        ids
    }
    unsafe fn rebuild(&mut self, keep: Option<Vec<Vec<u8>>>) {
        let selected: HashSet<Vec<u8>> = keep
            .unwrap_or_else(|| self.selected())
            .into_iter()
            .collect();
        let mut query = vec![0u16; 258];
        let n = GetWindowTextW(self.search, &mut query);
        let query = String::from_utf16_lossy(&query[..n as usize]).to_lowercase();
        self.visible = (0..self.items.len())
            .filter(|&i| {
                query.is_empty()
                    || self.items[i].name.to_lowercase().contains(&query)
                    || self.items[i].from.to_lowercase().contains(&query)
            })
            .collect();
        self.visible.sort_by(|&a, &b| {
            let a = &self.items[a];
            let b = &self.items[b];
            let order = match self.sort {
                0 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                1 => a.from.to_lowercase().cmp(&b.from.to_lowercase()),
                2 => a.size.cmp(&b.size),
                _ => a
                    .deleted
                    .map(|t| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64)
                    .cmp(
                        &b.deleted
                            .map(|t| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64),
                    ),
            };
            if self.descending {
                order.reverse()
            } else {
                order
            }
        });
        let reset = LVITEMW {
            state: LIST_VIEW_ITEM_STATE_FLAGS(0),
            stateMask: LVIS_SELECTED,
            ..Default::default()
        };
        SendMessageW(
            self.list,
            LVM_SETITEMSTATE,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM((&reset as *const LVITEMW) as isize)),
        );
        SendMessageW(
            self.list,
            LVM_SETITEMCOUNT,
            Some(WPARAM(self.visible.len())),
            None,
        );
        for (index, &item) in self.visible.iter().enumerate() {
            if selected.contains(&self.items[item].id) {
                let state = LVITEMW {
                    state: LVIS_SELECTED,
                    stateMask: LVIS_SELECTED,
                    ..Default::default()
                };
                SendMessageW(
                    self.list,
                    LVM_SETITEMSTATE,
                    Some(WPARAM(index)),
                    Some(LPARAM((&state as *const LVITEMW) as isize)),
                );
            }
        }
        self.buttons();
        self.fit_columns();
        let _ = InvalidateRect(Some(self.list), None, true);
    }
    unsafe fn tray(&mut self, add: bool) {
        let mut stock = SHSTOCKICONINFO {
            cbSize: size_of::<SHSTOCKICONINFO>() as u32,
            ..Default::default()
        };
        let _ = SHGetStockIconInfo(
            if self.summary.count == 0 {
                SIID_RECYCLER
            } else {
                SIID_RECYCLERFULL
            },
            SHGSI_ICON | SHGSI_SMALLICON,
            &mut stock,
        );
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_TIP | NIF_ICON,
            uCallbackMessage: TRAY,
            hIcon: stock.hIcon,
            ..Default::default()
        };
        let tip = if self.stats_ok {
            format!(
                "Recycle Desk\n{} · {}",
                text::count(self.settings.lang, self.summary.count),
                text::bytes(self.settings.lang, self.summary.bytes)
            )
        } else {
            "Recycle Desk".into()
        };
        let tip = wide(&tip);
        let len = tip.len().min(data.szTip.len() - 1);
        data.szTip[..len].copy_from_slice(&tip[..len]);
        if !Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &data).as_bool() {
            SetTimer(Some(self.hwnd), 2, 3000, None);
        } else {
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
            let _ = KillTimer(Some(self.hwnd), 2);
        }
        if !self.icon.is_invalid() {
            let _ = DestroyIcon(self.icon);
        }
        self.icon = stock.hIcon;
    }
    unsafe fn menu(&self, settings_menu: bool) -> Result<usize> {
        let menu = CreatePopupMenu()?;
        let append = |id, label: &str, checked: bool| -> Result<()> {
            AppendMenuW(
                menu,
                MF_STRING
                    | if checked {
                        MF_CHECKED
                    } else {
                        MENU_ITEM_FLAGS(0)
                    },
                id,
                PCWSTR(wide(label).as_ptr()),
            )
        };
        if settings_menu {
            AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR(wide(self.t(Key::Language)).as_ptr()),
            )?;
            append(LANG_UK, "Українська", self.settings.lang == Lang::Uk)?;
            append(LANG_EN, "English", self.settings.lang == Lang::En)?;
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR(wide(self.t(Key::Theme)).as_ptr()),
            )?;
            for (id, key, theme) in [
                (THEME_SYSTEM, Key::System, Theme::System),
                (THEME_LIGHT, Key::Light, Theme::Light),
                (THEME_DARK, Key::Dark, Theme::Dark),
            ] {
                append(id, self.t(key), self.settings.theme == theme)?;
            }
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            append(STARTUP, self.t(Key::Startup), settings::startup_enabled())?;
            append(
                AUTO_UPDATE,
                self.t(Key::AutoUpdate),
                self.settings.auto_update,
            )?;
            AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR(wide(self.t(self.update_status)).as_ptr()),
            )?;
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            append(EXPLORER, self.t(Key::Explorer), false)?;
            append(EXIT, self.t(Key::Exit), false)?;
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR(
                    wide(self.t(if self.settings.portable {
                        Key::Portable
                    } else {
                        Key::Installed
                    }))
                    .as_ptr(),
                ),
            )?;
            AppendMenuW(
                menu,
                MF_STRING | MF_DISABLED,
                0,
                PCWSTR(wide(self.t(Key::About)).as_ptr()),
            )?;
        } else {
            for (id, key) in [
                (OPEN as usize, Key::Title),
                (EXPLORER, Key::Explorer),
                (EMPTY, Key::Empty),
                (SETTINGS, Key::Settings),
                (EXIT, Key::Exit),
            ] {
                if id == EXIT && self.busy {
                    AppendMenuW(
                        menu,
                        MF_STRING | MF_DISABLED,
                        id,
                        PCWSTR(wide(self.t(key)).as_ptr()),
                    )?;
                } else {
                    append(id, self.t(key), false)?;
                }
            }
        }
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(self.hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            self.hwnd,
            None,
        )
        .0 as usize;
        let _ = DestroyMenu(menu);
        let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        Ok(selected)
    }
    unsafe fn handle(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) -> (Option<LRESULT>, Effect) {
        let mut effect = Effect::None;
        match msg {
            WM_CREATE => {
                self.taskbar_created = RegisterWindowMessageW(w!("TaskbarCreated"));
                self.dpi = GetDpiForWindow(self.hwnd) as i32;
                self.theme();
                self.tray(true);
                self.stats();
                if let Ok(pidl) = SHGetKnownFolderIDList(&FOLDERID_RecycleBinFolder, 0, None) {
                    let entry = SHChangeNotifyEntry {
                        pidl,
                        fRecursive: true.into(),
                    };
                    self.watch = SHChangeNotifyRegister(
                        self.hwnd,
                        SHCNRF_ShellLevel | SHCNRF_InterruptLevel | SHCNRF_NewDelivery,
                        SHCNE_ALLEVENTS.0 as i32,
                        CHANGE,
                        1,
                        &entry,
                    );
                    windows::Win32::System::Com::CoTaskMemFree(Some(pidl.cast()));
                }
                // Low-frequency fallback for unavailable or lost Shell notifications.
                SetTimer(
                    Some(self.hwnd),
                    3,
                    if self.watch == 0 { 15000 } else { 60000 },
                    None,
                );
                SetTimer(Some(self.hwnd), 4, updater::INTERVAL_MS, None);
                self.check_updates();
            }
            OPEN => {
                self.opening_window = false;
                if let Err(e) = self.show() {
                    effect = Effect::Error(e.to_string());
                }
            }
            APPLY_UPDATE => {}
            WM_CLOSE => self.hide(),
            WM_SIZE => {
                if wp.0 == SIZE_MINIMIZED as usize {
                    self.hide();
                } else {
                    self.layout();
                }
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lp.0 as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT {
                    x: self.px(760),
                    y: self.px(500),
                };
            }
            WM_DPICHANGED => {
                self.dpi = (wp.0 & 0xffff) as i32;
                self.fonts();
                let r = &*(lp.0 as *const RECT);
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                self.layout();
            }
            WM_SETTINGCHANGE | WM_THEMECHANGED => {
                self.theme();
            }
            CHANGE => {
                let mut first = std::ptr::null_mut();
                let mut event = 0;
                let lock = SHChangeNotification_Lock(
                    HANDLE(wp.0 as *mut _),
                    lp.0 as u32,
                    Some(&mut first),
                    Some(&mut event),
                );
                if !lock.is_invalid() {
                    let _ = SHChangeNotification_Unlock(lock);
                }
                SetTimer(Some(self.hwnd), 1, 300, None);
            }
            WM_TIMER => match wp.0 {
                1 => {
                    let _ = KillTimer(Some(self.hwnd), 1);
                    self.stats();
                    if !self.controls.is_empty() {
                        self.scan();
                    }
                }
                2 => self.tray(true),
                3 => self.stats(),
                4 => self.check_updates(),
                _ => {}
            },
            RESULT => {
                while let Ok(reply) = self.rx.try_recv() {
                    match reply {
                        Reply::Update(result) => {
                            self.update_busy = false;
                            if !self.settings.auto_update {
                                self.update_status = Key::UpdateDisabled;
                            } else {
                                match result {
                                    Ok(Some(staged)) => {
                                        self.staged = Some(staged);
                                        self.update_status = Key::UpdateReady;
                                    }
                                    Ok(None) => self.update_status = Key::UpdateCurrent,
                                    Err(_) => self.update_status = Key::UpdateFailed,
                                }
                            }
                        }
                        Reply::Stats(Ok(stats)) => {
                            self.summary = stats;
                            self.stats_ok = true;
                            self.update_summary();
                            self.tray(false);
                        }
                        Reply::Stats(Err(_)) => {
                            self.stats_ok = false;
                            self.buttons();
                        }
                        Reply::List(result) => {
                            self.busy = false;
                            if !self.controls.is_empty() {
                                match result {
                                    Ok(items) => {
                                        let keep = self.selected();
                                        self.items = items;
                                        self.view_error = false;
                                        self.rebuild(Some(keep));
                                        let status = self.last_status.take().unwrap_or(Key::Ready);
                                        set_text(self.status, self.t(status));
                                    }
                                    Err(error) => {
                                        self.items.clear();
                                        self.view_error = true;
                                        self.rebuild(Some(vec![]));
                                        set_text(self.status, self.t(Key::Failed));
                                        effect = Effect::Error(error);
                                    }
                                }
                            }
                            self.stats();
                            self.buttons();
                            if self.pending {
                                self.pending = false;
                                self.scan();
                            }
                        }
                        Reply::Operation(result) => {
                            self.busy = false;
                            if let Err(error) = &result {
                                effect = Effect::Error(error.clone());
                            }
                            self.last_status = Some(if matches!(result, Ok(true)) {
                                Key::Completed
                            } else {
                                Key::Partial
                            });
                            self.scan();
                        }
                    }
                }
            }
            WM_COMMAND => {
                if wp.0 & 0xffff == SEARCH && (wp.0 >> 16) as u32 == EN_CHANGE {
                    self.rebuild(None);
                } else {
                    effect = Effect::Command(wp.0 & 0xffff);
                }
            }
            WM_NOTIFY => {
                let hdr = &*(lp.0 as *const NMHDR);
                if hdr.hwndFrom == self.list {
                    match hdr.code {
                        LVN_GETEMPTYMARKUP => {
                            let info = &mut *(lp.0 as *mut NMLVEMPTYMARKUP);
                            let label = wide(if self.busy {
                                self.t(Key::Loading)
                            } else if self.view_error {
                                self.t(Key::Failed)
                            } else if self.items.is_empty() {
                                self.t(Key::EmptyState)
                            } else {
                                self.t(Key::NoMatch)
                            });
                            let len = (info.szMarkup.len() - 1).min(label.len() - 1);
                            info.szMarkup[..len].copy_from_slice(&label[..len]);
                            info.szMarkup[len] = 0;
                            return (Some(LRESULT(1)), effect);
                        }
                        LVN_GETDISPINFOW => {
                            let info = &mut *(lp.0 as *mut NMLVDISPINFOW);
                            if info.item.mask & LVIF_TEXT != LIST_VIEW_ITEM_FLAGS(0) {
                                if let Some(&index) = self.visible.get(info.item.iItem as usize) {
                                    let item = &self.items[index];
                                    let text = match info.item.iSubItem {
                                        0 => item.name.clone(),
                                        1 => item.from.clone(),
                                        2 => item
                                            .size
                                            .map(|s| text::bytes(self.settings.lang, s))
                                            .unwrap_or_else(|| "—".into()),
                                        _ => item
                                            .deleted
                                            .map(|t| shell::date(self.settings.lang, t))
                                            .unwrap_or_else(|| "—".into()),
                                    };
                                    if !info.item.pszText.is_null() && info.item.cchTextMax > 0 {
                                        let data = wide(&text);
                                        let len =
                                            (info.item.cchTextMax as usize - 1).min(data.len() - 1);
                                        std::ptr::copy_nonoverlapping(
                                            data.as_ptr(),
                                            info.item.pszText.0,
                                            len,
                                        );
                                        *info.item.pszText.0.add(len) = 0;
                                    }
                                }
                            }
                        }
                        LVN_COLUMNCLICK => {
                            let col = (*(lp.0 as *const NMLISTVIEW)).iSubItem as usize;
                            if col == self.sort {
                                self.descending = !self.descending;
                            } else {
                                self.sort = col;
                                self.descending = false;
                            }
                            self.rebuild(None);
                        }
                        LVN_ITEMCHANGED | LVN_ODSTATECHANGED => {
                            self.buttons();
                        }
                        LVN_KEYDOWN => {
                            let key = (*(lp.0 as *const NMLVKEYDOWN)).wVKey;
                            if key == VK_DELETE.0 {
                                effect = Effect::Command(DELETE);
                            }
                        }
                        _ => {}
                    }
                }
            }
            TRAY => match lp.0 as u32 & 0xffff {
                WM_LBUTTONUP | NIN_SELECT | TRAY_KEYSELECT => {
                    if let Err(e) = self.show() {
                        effect = Effect::Error(e.to_string());
                    }
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    effect = Effect::Command(0);
                }
                _ => {}
            },
            WM_DESTROY => {
                self.shutting_down = true;
                if self.watch != 0 {
                    let _ = SHChangeNotifyDeregister(self.watch);
                }
                let data = NOTIFYICONDATAW {
                    cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: self.hwnd,
                    uID: 1,
                    ..Default::default()
                };
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                if !self.icon.is_invalid() {
                    let _ = DestroyIcon(self.icon);
                }
                if !self.font.is_invalid() {
                    let _ = DeleteObject(self.font.into());
                }
                if !self.title_font.is_invalid() {
                    let _ = DeleteObject(self.title_font.into());
                }
                if !self.brush.is_invalid() {
                    let _ = DeleteObject(self.brush.into());
                }
                PostQuitMessage(0);
            }
            WM_QUERYENDSESSION => {
                self.shutting_down = true;
                return (Some(LRESULT(1)), effect);
            }
            WM_ENDSESSION => {
                if wp.0 == 0 {
                    self.shutting_down = false;
                }
            }
            _ if msg == self.taskbar_created && self.taskbar_created != 0 => self.tray(true),
            _ => return (None, effect),
        }
        if !self.shutting_down
            && matches!(effect, Effect::None)
            && self.staged.is_some()
            && updater::can_apply(
                self.settings.auto_update,
                !self.controls.is_empty() || self.opening_window,
                self.busy,
                window_state(self.hwnd).modal_depth.get() > 0,
            )
        {
            effect = Effect::InstallUpdate;
        }
        (Some(LRESULT(0)), effect)
    }
}

unsafe fn window_state(hwnd: HWND) -> &'static WindowState {
    &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState)
}
unsafe fn app_cell(hwnd: HWND) -> &'static RefCell<App> {
    &window_state(hwnd).app
}

struct ModalGuard(HWND);
impl ModalGuard {
    unsafe fn new(hwnd: HWND) -> Self {
        let depth = &window_state(hwnd).modal_depth;
        depth.set(depth.get() + 1);
        Self(hwnd)
    }
}
impl Drop for ModalGuard {
    fn drop(&mut self) {
        unsafe {
            if !IsWindow(Some(self.0)).as_bool() {
                return;
            }
            let depth = &window_state(self.0).modal_depth;
            depth.set(depth.get().saturating_sub(1));
            let _ = PostMessageW(Some(self.0), APPLY_UPDATE, WPARAM(0), LPARAM(0));
        }
    }
}

unsafe fn install_update(hwnd: HWND) {
    let staged = app_cell(hwnd).borrow_mut().staged.take();
    if let Some(staged) = staged {
        match staged
            .install()
            .and_then(|_| updater::relaunch(&[std::ffi::OsStr::new("--tray")]))
        {
            Ok(()) => {
                let _ = DestroyWindow(hwnd);
            }
            Err(_) => app_cell(hwnd).borrow_mut().update_status = Key::UpdateFailed,
        }
    }
}

unsafe fn paint_window(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    let paint = window_state(hwnd).paint.get();
    let p = paint.palette;
    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            let hdc = HDC(wp.0 as *mut _);
            SetTextColor(hdc, p.text);
            SetBkColor(hdc, p.base);
            Some(LRESULT(paint.brush.0 as isize))
        }
        WM_ERASEBKGND if !paint.brush.is_invalid() => {
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            FillRect(HDC(wp.0 as *mut _), &rect, paint.brush);
            Some(LRESULT(1))
        }
        WM_DRAWITEM => {
            let draw = &*(lp.0 as *const DRAWITEMSTRUCT);
            let primary = draw.CtlID as usize == RESTORE;
            let disabled = draw.itemState.0 & ODS_DISABLED.0 != 0;
            let brush = CreateSolidBrush(if primary && !disabled {
                p.accent
            } else {
                p.surface
            });
            FillRect(draw.hDC, &draw.rcItem, brush);
            let _ = DeleteObject(brush.into());
            SetBkMode(draw.hDC, TRANSPARENT);
            SetTextColor(
                draw.hDC,
                if disabled {
                    p.muted
                } else if primary {
                    p.on_accent
                } else if [DELETE, EMPTY].contains(&(draw.CtlID as usize)) {
                    p.danger
                } else {
                    p.text
                },
            );
            let old = SelectObject(draw.hDC, paint.font.into());
            let mut label = [0u16; 160];
            let n = GetWindowTextW(draw.hwndItem, &mut label);
            let mut rect = draw.rcItem;
            if draw.itemState.0 & ODS_SELECTED.0 != 0 {
                rect.left += 1;
                rect.top += 1;
            }
            let hide = if draw.itemState.0 & ODS_NOACCEL.0 != 0 {
                DT_HIDEPREFIX
            } else {
                DRAW_TEXT_FORMAT(0)
            };
            DrawTextW(
                draw.hDC,
                &mut label[..n as usize],
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | hide,
            );
            if draw.itemState.0 & ODS_FOCUS.0 != 0 {
                rect.left += 3;
                rect.top += 3;
                rect.right -= 3;
                rect.bottom -= 3;
                let _ = DrawFocusRect(draw.hDC, &rect);
            }
            SelectObject(draw.hDC, old);
            Some(LRESULT(1))
        }
        WM_NOTIFY => {
            let hdr = &*(lp.0 as *const NMHDR);
            if hdr.hwndFrom != paint.header || hdr.code != NM_CUSTOMDRAW {
                return None;
            }
            let draw = &*(lp.0 as *const NMCUSTOMDRAW);
            match draw.dwDrawStage {
                CDDS_PREPAINT => Some(LRESULT(CDRF_NOTIFYITEMDRAW as isize)),
                CDDS_ITEMPREPAINT => {
                    let brush = CreateSolidBrush(p.surface);
                    FillRect(draw.hdc, &draw.rc, brush);
                    let _ = DeleteObject(brush.into());
                    let mut label = [0u16; 256];
                    let mut item = HDITEMW {
                        mask: HDI_TEXT | HDI_FORMAT,
                        pszText: PWSTR(label.as_mut_ptr()),
                        cchTextMax: label.len() as i32,
                        ..Default::default()
                    };
                    SendMessageW(
                        paint.header,
                        HDM_GETITEMW,
                        Some(WPARAM(draw.dwItemSpec)),
                        Some(LPARAM((&mut item as *mut HDITEMW) as isize)),
                    );
                    let mut rect = draw.rc;
                    let pad = 8 * paint.dpi / 96;
                    rect.left += pad;
                    rect.right -= pad;
                    SetBkMode(draw.hdc, TRANSPARENT);
                    SetTextColor(draw.hdc, p.text);
                    let old = SelectObject(draw.hdc, paint.font.into());
                    let align = if item.fmt.0 & HDF_RIGHT.0 != 0 {
                        DT_RIGHT
                    } else {
                        DT_LEFT
                    };
                    let len = label.iter().position(|&c| c == 0).unwrap_or(label.len());
                    DrawTextW(
                        draw.hdc,
                        &mut label[..len],
                        &mut rect,
                        align | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                    SelectObject(draw.hdc, old);
                    Some(LRESULT(CDRF_SKIPDEFAULT as isize))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

unsafe fn report(hwnd: HWND, error: &str) {
    let _modal = ModalGuard::new(hwnd);
    let lang = app_cell(hwnd).borrow().settings.lang;
    MessageBoxW(
        Some(hwnd),
        PCWSTR(wide(&format!("{}\n\n{error}", text::tr(lang, Key::Failed))).as_ptr()),
        w!("Recycle Desk"),
        MB_OK | MB_ICONERROR,
    );
}

unsafe fn command(hwnd: HWND, id: usize) {
    // Native menus/dialogs run nested message loops. Release the RefCell borrow
    // first so paint and notification callbacks can continue safely.
    if id == 0 || id == SETTINGS {
        let _modal = ModalGuard::new(hwnd);
        let result = {
            let app = app_cell(hwnd).borrow();
            app.menu(id == SETTINGS)
        };
        match result {
            Ok(next) if next != 0 => command(hwnd, next),
            Err(e) => report(hwnd, &e.to_string()),
            _ => {}
        }
        return;
    }
    if id == DELETE || id == EMPTY {
        let _modal = ModalGuard::new(hwnd);
        let (lang, ids, summary, busy) = {
            let app = app_cell(hwnd).borrow();
            (app.settings.lang, app.selected(), app.summary, app.busy)
        };
        if busy || (id == DELETE && ids.is_empty()) {
            return;
        }
        let n = if id == EMPTY {
            summary.count
        } else {
            ids.len() as u64
        };
        let message = format!(
            "{}\n\n{}\n{}",
            text::tr(
                lang,
                if id == EMPTY {
                    Key::ConfirmEmpty
                } else {
                    Key::ConfirmDelete
                }
            ),
            text::count(lang, n),
            text::tr(lang, Key::Irreversible)
        );
        if MessageBoxW(
            Some(hwnd),
            PCWSTR(wide(&message).as_ptr()),
            w!("Recycle Desk"),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) != IDYES
        {
            return;
        }
        let mut app = app_cell(hwnd).borrow_mut();
        if app.busy {
            return;
        }
        app.busy = true;
        app.buttons();
        let owner = hwnd.0 as usize;
        app.send_work(move || {
            Reply::Operation(
                if id == EMPTY {
                    shell::empty(owner).map(|_| true)
                } else {
                    shell::act(owner, &ids, Action::Delete)
                }
                .map_err(|e| e.to_string()),
            )
        });
        return;
    }
    if id == RESTORE {
        let mut app = app_cell(hwnd).borrow_mut();
        if app.busy {
            return;
        }
        let ids = app.selected();
        if ids.is_empty() {
            return;
        }
        app.busy = true;
        app.buttons();
        let owner = hwnd.0 as usize;
        app.send_work(move || {
            Reply::Operation(shell::act(owner, &ids, Action::Restore).map_err(|e| e.to_string()))
        });
        return;
    }
    if id == EXIT {
        if !app_cell(hwnd).borrow().busy {
            let _ = DestroyWindow(hwnd);
        }
        return;
    }
    if id == EXPLORER {
        ShellExecuteW(
            Some(hwnd),
            w!("open"),
            w!("shell:RecycleBinFolder"),
            None,
            None,
            SW_SHOWNORMAL,
        );
        return;
    }
    if id == OPEN as usize {
        let _ = PostMessageW(Some(hwnd), OPEN, WPARAM(0), LPARAM(0));
        return;
    }
    let result = {
        let mut app = app_cell(hwnd).borrow_mut();
        match id {
            REFRESH => {
                app.scan();
                Ok(())
            }
            STARTUP => {
                settings::set_startup(!settings::startup_enabled()).map_err(|e| e.to_string())
            }
            AUTO_UPDATE => {
                let previous = app.settings.auto_update;
                app.settings.auto_update = !previous;
                match app.settings.save() {
                    Err(e) => {
                        app.settings.auto_update = previous;
                        Err(e.to_string())
                    }
                    Ok(()) => {
                        if !app.settings.auto_update {
                            app.staged = None;
                        }
                        app.check_updates();
                        Ok(())
                    }
                }
            }
            LANG_UK | LANG_EN | THEME_SYSTEM | THEME_LIGHT | THEME_DARK => {
                let was_visible = !app.controls.is_empty();
                match id {
                    LANG_UK => app.settings.lang = Lang::Uk,
                    LANG_EN => app.settings.lang = Lang::En,
                    THEME_SYSTEM => app.settings.theme = Theme::System,
                    THEME_LIGHT => app.settings.theme = Theme::Light,
                    _ => app.settings.theme = Theme::Dark,
                }
                let result = app.settings.save().map_err(|e| e.to_string());
                if was_visible {
                    app.hide();
                    if let Err(e) = app.show() {
                        return {
                            drop(app);
                            report(hwnd, &e.to_string());
                        };
                    }
                }
                app.theme();
                app.tray(false);
                result
            }
            _ => Ok(()),
        }
    };
    if let Err(error) = result {
        report(hwnd, &error);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(lp.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        let state = &*(create.lpCreateParams as *const WindowState);
        state.app.borrow_mut().hwnd = hwnd;
    }
    if GetWindowLongPtrW(hwnd, GWLP_USERDATA) == 0 {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    if let Some(result) = paint_window(hwnd, msg, wp, lp) {
        return result;
    }
    let cell = app_cell(hwnd);
    let (result, effect) = if let Ok(mut app) = cell.try_borrow_mut() {
        app.handle(msg, wp, lp)
    } else {
        return DefWindowProcW(hwnd, msg, wp, lp);
    };
    match effect {
        Effect::None => {}
        Effect::Command(id) => command(hwnd, id),
        Effect::Error(e) => report(hwnd, &e),
        Effect::InstallUpdate => install_update(hwnd),
    }
    result.unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp))
}

pub fn run() -> Result<()> {
    unsafe {
        settings::refresh_installed_version();
        let _com = shell::Com::new()?;
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(None, false, w!("Local\\RecycleDesk.v1"))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(hwnd) = FindWindowW(CLASS, None) {
                let _ = PostMessageW(Some(hwnd), OPEN, WPARAM(0), LPARAM(0));
            }
            let _ = CloseHandle(mutex);
            return Ok(());
        }
        if !InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES,
        })
        .as_bool()
        {
            return Err(Error::from_win32());
        }
        let module = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            hIcon: LoadIconW(Some(module.into()), PCWSTR(1usize as *const u16))?,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(Error::from_win32());
        }
        let (tx, rx) = mpsc::channel();
        let app = App::new(tx, rx);
        let paint = Paint {
            palette: app.palette,
            brush: HBRUSH::default(),
            font: HFONT::default(),
            header: HWND::default(),
            dpi: 96,
        };
        let state = Box::new(WindowState {
            app: RefCell::new(app),
            paint: Cell::new(paint),
            modal_depth: Cell::new(0),
        });
        let dpi = GetDpiForSystem() as i32;
        let mut work = RECT::default();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut work as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )?;
        let width = (900 * dpi / 96).min(work.right - work.left - 48);
        let height = (640 * dpi / 96).min(work.bottom - work.top - 48);
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            CLASS,
            w!("Recycle Desk"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            work.left + (work.right - work.left - width) / 2,
            work.top + (work.bottom - work.top - height) / 2,
            width,
            height,
            None,
            None,
            Some(module.into()),
            Some((&*state as *const WindowState).cast()),
        )?;
        if !std::env::args().any(|a| a == "--tray") {
            let _ = PostMessageW(Some(hwnd), OPEN, WPARAM(0), LPARAM(0));
        }
        let mut msg = MSG::default();
        loop {
            let result = GetMessageW(&mut msg, None, 0, 0).0;
            if result == 0 {
                break;
            }
            if result < 0 {
                return Err(Error::from_win32());
            }
            let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
            if msg.message == WM_KEYDOWN {
                if msg.wParam.0 == VK_F5.0 as usize {
                    command(hwnd, REFRESH);
                    continue;
                }
                if msg.wParam.0 == VK_ESCAPE.0 as usize {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    continue;
                }
                if ctrl && msg.wParam.0 == b'F' as usize {
                    let search = state.app.borrow().search;
                    let _ = SetFocus(Some(search));
                    continue;
                }
                if ctrl && msg.wParam.0 == b'Q' as usize {
                    command(hwnd, EXIT);
                    continue;
                }
                if ctrl && msg.wParam.0 == b'A' as usize {
                    let app = state.app.borrow();
                    if GetFocus() == app.list {
                        let item = LVITEMW {
                            state: LVIS_SELECTED,
                            stateMask: LVIS_SELECTED,
                            ..Default::default()
                        };
                        SendMessageW(
                            app.list,
                            LVM_SETITEMSTATE,
                            Some(WPARAM(usize::MAX)),
                            Some(LPARAM((&item as *const LVITEMW) as isize)),
                        );
                        continue;
                    }
                }
            }
            if IsDialogMessageW(hwnd, &msg).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = CloseHandle(mutex);
        Ok(())
    }
}
