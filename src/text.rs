#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Uk,
    En,
}

#[derive(Clone, Copy)]
pub enum Key {
    Title,
    Subtitle,
    Search,
    Refresh,
    Restore,
    Delete,
    Empty,
    Explorer,
    Settings,
    Exit,
    Name,
    From,
    Size,
    Date,
    Loading,
    Ready,
    EmptyState,
    NoMatch,
    Failed,
    ConfirmDelete,
    ConfirmEmpty,
    Irreversible,
    Completed,
    Partial,
    Selection,
    Language,
    Theme,
    System,
    Light,
    Dark,
    Startup,
    Portable,
    Installed,
    About,
    AutoUpdate,
    UpdateUnconfigured,
    UpdateChecking,
    UpdateReady,
    UpdateCurrent,
    UpdateFailed,
    UpdateDisabled,
}

pub fn tr(lang: Lang, key: Key) -> &'static str {
    use Key::*;
    let pair = match key {
        Title => ("Кошик", "Recycle bin"),
        Subtitle => (
            "Перегляньте. Відновіть. Звільніть місце.",
            "Review. Restore. Make room.",
        ),
        Search => ("Пошук за назвою або шляхом", "Search name or original path"),
        Refresh => ("&Оновити", "&Refresh"),
        Restore => ("&Відновити", "Res&tore"),
        Delete => ("В&идалити", "&Delete"),
        Empty => ("О&чистити все", "&Empty all"),
        Explorer => ("Відкрити в &Провіднику", "Open in &Explorer"),
        Settings => ("&Налаштування", "&Settings"),
        Exit => ("Ви&йти", "E&xit"),
        Name => ("Назва", "Name"),
        From => ("Початкове розташування", "Original location"),
        Size => ("Розмір", "Size"),
        Date => ("Дата видалення", "Date deleted"),
        Loading => ("Читання кошика…", "Reading recycle bin…"),
        Ready => (
            "Закриття вікна залишає програму в треї.",
            "Closing this window keeps the app in the tray.",
        ),
        EmptyState => ("Кошик порожній", "Your recycle bin is empty"),
        NoMatch => ("Нічого не знайдено", "No matching items"),
        Failed => (
            "Не вдалося виконати операцію",
            "The operation could not be completed",
        ),
        ConfirmDelete => (
            "Остаточно видалити вибрані елементи?",
            "Permanently delete the selected items?",
        ),
        ConfirmEmpty => (
            "Очистити весь кошик на всіх дисках?",
            "Empty the entire recycle bin on all drives?",
        ),
        Irreversible => (
            "Цю дію неможливо скасувати.",
            "This action cannot be undone.",
        ),
        Completed => ("Операцію завершено", "Operation completed"),
        Partial => (
            "Частину елементів не оброблено. Перевірте список.",
            "Some items were not processed. Please review the list.",
        ),
        Selection => ("Вибрано", "Selected"),
        Language => ("Мова", "Language"),
        Theme => ("Оформлення", "Appearance"),
        System => ("Як у системі", "Follow Windows"),
        Light => ("Світле · Latte", "Light · Latte"),
        Dark => ("Темне · Mocha", "Dark · Mocha"),
        Startup => ("Запускати разом із Windows", "Start with Windows"),
        Portable => (
            "Переносний режим · налаштування поряд із програмою",
            "Portable mode · settings beside the app",
        ),
        Installed => (
            "Налаштування зберігаються в LocalAppData",
            "Settings are stored in LocalAppData",
        ),
        About => (
            concat!(
                "Recycle Desk ",
                env!("CARGO_PKG_VERSION"),
                " · MIT · Rust + Win32"
            ),
            concat!(
                "Recycle Desk ",
                env!("CARGO_PKG_VERSION"),
                " · MIT · Rust + Win32"
            ),
        ),
        AutoUpdate => (
            "Автоматично оновлювати програму",
            "Update the app automatically",
        ),
        UpdateUnconfigured => (
            "Автооновлення ще не налаштоване для цієї збірки",
            "Updates are not configured for this build",
        ),
        UpdateChecking => (
            "Перевірка та завантаження оновлення…",
            "Checking and downloading updates…",
        ),
        UpdateReady => (
            "Оновлення готове. Закрийте вікно в трей для встановлення.",
            "Update ready. Close this window to the tray to install.",
        ),
        UpdateCurrent => (
            "Встановлено актуальну версію",
            "You have the latest version",
        ),
        UpdateFailed => (
            "Оновлення не встановлено. Повторимо перевірку пізніше.",
            "Update was not installed. We will check again later.",
        ),
        UpdateDisabled => ("Автооновлення вимкнено", "Automatic updates are off"),
    };
    if lang == Lang::Uk {
        pair.0
    } else {
        pair.1
    }
}

pub fn count(lang: Lang, n: u64) -> String {
    let unit = if lang == Lang::En {
        if n == 1 {
            "item"
        } else {
            "items"
        }
    } else if n % 10 == 1 && n % 100 != 11 {
        "елемент"
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        "елементи"
    } else {
        "елементів"
    };
    format!("{n} {unit}")
}

pub fn bytes(lang: Lang, n: u64) -> String {
    let units = if lang == Lang::Uk {
        ["Б", "КіБ", "МіБ", "ГіБ", "ТіБ", "ПіБ"]
    } else {
        ["B", "KiB", "MiB", "GiB", "TiB", "PiB"]
    };
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let number = if unit == 0 {
        n.to_string()
    } else {
        format!("{value:.1}")
    };
    let number = if lang == Lang::Uk {
        number.replace('.', ",")
    } else {
        number
    };
    format!("{number} {}", units[unit])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ukrainian_plural_and_sizes() {
        for (n, s) in [
            (1, "1 елемент"),
            (2, "2 елементи"),
            (11, "11 елементів"),
            (14, "14 елементів"),
            (21, "21 елемент"),
            (112, "112 елементів"),
        ] {
            assert_eq!(count(Lang::Uk, n), s);
        }
        assert_eq!(bytes(Lang::Uk, 1536), "1,5 КіБ");
        assert_eq!(bytes(Lang::En, 1024), "1.0 KiB");
    }
}
