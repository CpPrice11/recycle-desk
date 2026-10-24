# Recycle Desk

A compact native Windows recycle bin manager written independently in Rust and Win32. Free and open source under MIT.

[Download the latest release](https://github.com/CpPrice11/recycle-desk/releases/latest) · [Українська](README.md)

Version 0.1.2 adds inline file thumbnails, a branded tray icon with a full-bin indicator, and corrected dark table-header painting. Visual validation remains incomplete; see the [build log](replica/build-log.md).

## Features

- Tray icon with item count and total size.
- File list with original locations, sizes and deletion dates; search and sort.
- Restore selected items through Windows Shell.
- Permanently delete selected items or empty the recycle bin, with confirmation.
- English and Ukrainian; Catppuccin Mocha and Latte themes.
- Optional startup, portable settings, no telemetry or automatic cleanup.
- Automatic updates from stable GitHub Releases, with pinned Ed25519ph signatures.

## Install

Target: Windows 10 22H2 / Windows 11, x64. Windows 10 has not yet been tested on a physical machine. No .NET or Python installation is needed.

**Portable:** extract `RecycleDesk-<version>-windows-x64.zip` into a writable folder and run `recycle-desk.exe`. Keep `portable.flag` beside it. Settings are stored in `recycle-desk.ini`.

**Installer:** run `RecycleDesk-<version>-windows-x64-setup.exe`. Installation is per user and requires no administrator rights. Settings are stored in `%LOCALAPPDATA%\RecycleDesk\settings.ini` and retained on uninstall.

Click the tray icon to open the window. Close the window or press Esc to return to the tray. Use the tray menu or Ctrl+Q to exit. Ctrl+F searches, F5 refreshes, and Delete requests permanent deletion of selected items. Restore conflicts are handled by Windows dialogs.

The EXE and installer do not have Authenticode certificates. Signed update archives use a separate signature verified by the application. `SHA256SUMS.txt` contains download checksums.

## Automatic updates

The app checks stable releases at startup and every six hours, downloads a signed package in the background, and installs it only when the window is hidden in the tray and file operations and dialogs have finished. It restarts automatically and preserves settings. Updates can be disabled in Settings. Network or signature failures leave the current executable in place.

The `*-update.zip` asset is for the updater. Use the portable ZIP or installer for a first installation.

## Build and release

Use stable Rust MSVC and Visual Studio Build Tools / Windows SDK. Run `cargo test --locked`, then `./scripts/build.ps1`. The release build includes the static C runtime. Run `$compiler = ./scripts/setup-inno.ps1` and `./scripts/package.ps1 -Iscc $compiler` to create packages.

GitHub Actions embeds the current repository and update public key. Tagged builds create a draft release with the portable ZIP, installer, signed update ZIP and checksums. The private signing key belongs only in the `RECYCLE_DESK_SIGNING_KEY` Actions secret. See [updater documentation](replica/updater.md) and [validation record](replica/test-plan.md).

## Credits

Code: [MIT](LICENSE). Dependency licenses: [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt). Colors: [Catppuccin](https://github.com/catppuccin/palette). File thumbnails, file-type icons and Segoe UI come from Windows; the font is not redistributed. The app and tray icon was generated with `imagegen` from an original prompt; [provenance](resources/icons/README.md). Its full-bin indicator is drawn at runtime. No Minibin source code or brand assets were copied.
