# Recycle Desk

A compact Windows recycle bin manager that lives in the system tray. Free and open source.

[Download](https://github.com/CpPrice11/recycle-desk/releases/latest) · [Українська](README.md)

## Screenshots

Version 0.1.2 with demo files.

![Dark theme with file thumbnails](https://raw.githubusercontent.com/CpPrice11/pullora-assets/main/media/cpprice11/recycle-desk/main-dark-uk-v012.png)

<details>
<summary>Light theme · English</summary>

![Light theme with file thumbnails](https://raw.githubusercontent.com/CpPrice11/pullora-assets/main/media/cpprice11/recycle-desk/main-light-en-v012.png)

</details>

<details>
<summary>Search the file list</summary>

![Searching by name filters the file list](https://raw.githubusercontent.com/CpPrice11/pullora-assets/main/media/cpprice11/recycle-desk/search-dark-uk-v012.png)

</details>

## Features

- Empty/full tray icon with item count and total size in its tooltip.
- File list with thumbnails, original locations, sizes and deletion dates.
- Search by name or path; sort by column.
- Restore selected files; permanently delete selected items or empty the bin with confirmation.
- English and Ukrainian, light and dark themes, and automatic theme selection from Windows.
- Optional startup, portable mode and automatic updates.

No telemetry or automatic cleanup. File operations work offline.

## Install

Target: **Windows 10 22H2 / Windows 11, x64**. No additional runtime installation is required.

- **Portable ZIP:** extract into a writable folder and run `recycle-desk.exe`. Keep `portable.flag` beside it to store settings in `recycle-desk.ini`.
- **Installer:** run `*-setup.exe`. Installation is per user and requires no administrator rights. Settings are stored in `%LOCALAPPDATA%\RecycleDesk\settings.ini` and retained on uninstall.

The `*-update.zip` file is for automatic updates. Choose the portable ZIP or installer for a first installation. Download checksums are in `SHA256SUMS.txt` alongside the release files.

## Use

Click the tray icon to open the window. Closing the window or pressing **Esc** returns the app to the tray. Choose **Exit** from the tray menu or press **Ctrl+Q** to quit.

**Ctrl+F** searches, **F5** refreshes, **Ctrl+A** in the list selects all items, and **Delete** requests permanent deletion with confirmation. Windows dialogs handle name conflicts during restoration.

[Hide the desktop Recycle Bin — guide with screenshots](docs/hide-desktop-recycle-bin.en.md)

## Automatic updates

Recycle Desk checks stable GitHub Releases at startup and every six hours. It downloads a package in the background, verifies its signature, and installs it when the window is hidden in the tray and file operations and dialogs have finished. Settings are preserved after restart.

Updates can be disabled in Settings. A download or signature failure leaves the current version in place.

## Limitations

The EXE and installer are not yet Authenticode-signed. Update packages use a separate signature verified by the app. Windows 10, the complete visual flow and mixed scaling across multiple monitors still require validation.

## Development

[Build, test and release updates](docs/development.md)

## License

Code and app icon: [MIT](LICENSE). Dependency licenses: [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt). Colors: [Catppuccin](https://github.com/catppuccin/palette). Windows supplies file thumbnails, file-type icons and Segoe UI; the font is not redistributed. Illustration sources are credited in the guide.
