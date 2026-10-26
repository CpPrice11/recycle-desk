# Runs only on a disposable GitHub Windows runner. Never use the local recycle bin.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:CI_SCREENSHOT_CAPTURE -ne 'true') {
    throw 'Screenshot capture is restricted to the dedicated GitHub Actions workflow.'
}
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName Microsoft.VisualBasic
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CaptureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct IconId { public uint Size; public IntPtr Window; public uint Id; public Guid Guid; }
    [DllImport("shell32.dll")] public static extern int Shell_NotifyIconGetRect(ref IconId id, out Rect rect);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool RedrawWindow(IntPtr window, IntPtr rect, IntPtr region, uint flags);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr window, int x, int y, int width, int height, bool repaint);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wparam, string value);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
}
'@
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'test-output/screenshots'
New-Item -ItemType Directory -Force $out | Out-Null
$exe = (Get-ChildItem (Join-Path $root 'test-output/portable') -Filter recycle-desk.exe -Recurse | Select-Object -First 1).FullName
if (-not $exe) { throw 'Portable application not found' }
$programDir = Split-Path $exe -Parent
if ([CaptureWindow]::FindWindow('Shell_TrayWnd', $null) -eq [IntPtr]::Zero) {
    Start-Process -FilePath "$env:WINDIR/explorer.exe" -WindowStyle Hidden
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        Start-Sleep -Milliseconds 250
        if ([CaptureWindow]::FindWindow('Shell_TrayWnd', $null) -ne [IntPtr]::Zero) { break }
    }
}
$demo = 'C:\Recycle Desk Demo'
if (Test-Path -LiteralPath $demo) { throw 'Demo directory already exists' }
New-Item -ItemType Directory "$demo/Images", "$demo/Documents" | Out-Null
foreach ($color in @('Blue', 'Green', 'Orange')) {
    $bitmap = [Drawing.Bitmap]::new(480, 300)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([Drawing.Color]::FromArgb(30, 30, 46))
        $brush = [Drawing.SolidBrush]::new([Drawing.Color]::FromName($color))
        try {
            $graphics.FillEllipse($brush, 50, 40, 220, 220)
            $graphics.FillRectangle([Drawing.Brushes]::LightSteelBlue, 300, 70, 120, 160)
            $bitmap.Save("$demo/Images/$color shapes.png", [Drawing.Imaging.ImageFormat]::Png)
        } finally { $brush.Dispose() }
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
[IO.File]::WriteAllText("$demo/Documents/Reading list.txt", 'Demo document for the Recycle Desk gallery.')
[IO.File]::WriteAllText("$demo/Documents/Project notes.txt", 'Example files only. No personal data.')
Compress-Archive -LiteralPath "$demo/Documents/Project notes.txt" -DestinationPath "$demo/Documents/Notes archive.zip"
Get-ChildItem -LiteralPath "$demo/Images", "$demo/Documents" -File | ForEach-Object {
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($_.FullName, 'OnlyErrorDialogs', 'SendToRecycleBin')
}
function Save-AppWindow([IntPtr]$window, [string]$name) {
    $rect = [CaptureWindow+Rect]::new()
    if (-not [CaptureWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Window bounds unavailable' }
    $image = [Drawing.Bitmap]::new($rect.Right - $rect.Left, $rect.Bottom - $rect.Top, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $graphics = [Drawing.Graphics]::FromImage($image)
    try {
        $dc = $graphics.GetHdc()
        try { if (-not [CaptureWindow]::PrintWindow($window, $dc, 2)) { throw 'Window capture failed' } }
        finally { $graphics.ReleaseHdc($dc) }
        $colors = [Collections.Generic.HashSet[int]]::new()
        for ($y = 50; $y -lt $image.Height; $y += 20) {
            for ($x = 20; $x -lt $image.Width; $x += 20) { [void]$colors.Add($image.GetPixel($x, $y).ToArgb()) }
        }
        if ($colors.Count -lt 12) { throw 'Captured window appears blank' }
        $image.Save((Join-Path $out $name), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $image.Dispose() }
}
function Save-DesktopImage([string]$name, [Drawing.Rectangle]$screen = [Drawing.Rectangle]::Empty) {
    if ($screen.IsEmpty) { $screen = [Windows.Forms.Screen]::PrimaryScreen.Bounds }
    $image = [Drawing.Bitmap]::new($screen.Width, $screen.Height, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $graphics = [Drawing.Graphics]::FromImage($image)
    try {
        $graphics.CopyFromScreen($screen.X, $screen.Y, 0, 0, $screen.Size)
        $image.Save((Join-Path $out $name), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $image.Dispose() }
}
function Save-TrayDesktop([IntPtr]$window) {
    $taskbar = [CaptureWindow]::FindWindow('Shell_TrayWnd', $null)
    if ($taskbar -eq [IntPtr]::Zero) { throw 'Remote Windows taskbar not available' }
    $notify = [CaptureWindow]::FindWindowEx($taskbar, [IntPtr]::Zero, 'TrayNotifyWnd', $null)
    if ($notify -eq [IntPtr]::Zero) { throw 'Notification area not available' }
    [void][CaptureWindow]::ShowWindow($window, 3)
    [void][CaptureWindow]::RedrawWindow($window, [IntPtr]::Zero, [IntPtr]::Zero, 0x185)
    Start-Sleep -Seconds 2
    Save-DesktopImage 'tray-diagnostic.png'
    $button = [CaptureWindow]::GetDlgItem($notify, 1502)
    if ($button -ne [IntPtr]::Zero) {
        [void][CaptureWindow]::PostMessage($button, 0xF5, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Seconds 1
        Save-DesktopImage 'overflow-diagnostic.png'
    }
    # Ask Windows for this exact app's registered notification icon (HWND + ID 1).
    # https://learn.microsoft.com/windows/win32/api/shellapi/nf-shellapi-shell_notifyicongetrect
    $identity = [CaptureWindow+IconId]::new()
    $identity.Size = [Runtime.InteropServices.Marshal]::SizeOf($identity)
    $identity.Window = $window
    $identity.Id = 1
    $bounds = [CaptureWindow+Rect]::new()
    if ([CaptureWindow]::Shell_NotifyIconGetRect([ref]$identity, [ref]$bounds) -ne 0) { throw 'Registered Recycle Desk tray icon not found' }
    $screen = [Windows.Forms.Screen]::PrimaryScreen.Bounds
    if ($bounds.Right - $bounds.Left -lt 8 -or $bounds.Bottom - $bounds.Top -lt 8 -or $bounds.Left -lt 0 -or $bounds.Top -lt 0 -or $bounds.Right -gt $screen.Right -or $bounds.Bottom -gt $screen.Bottom) { throw 'Tray icon is outside the captured screen' }
    [void][CaptureWindow]::SetCursorPos([int](($bounds.Left + $bounds.Right) / 2), [int](($bounds.Top + $bounds.Bottom) / 2))
    Start-Sleep -Milliseconds 1800
    Save-DesktopImage 'tray-desktop-uk-v012.png'
    $detail = [Drawing.Rectangle]::FromLTRB([Math]::Max(0, $bounds.Left - 250), [Math]::Max(0, $bounds.Top - 130), $screen.Right, $screen.Bottom)
    Save-DesktopImage 'tray-detail-uk-v012.png' $detail
    [ordered]@{ name='Recycle Desk'; x=$bounds.Left; y=$bounds.Top; width=($bounds.Right-$bounds.Left); height=($bounds.Bottom-$bounds.Top); desktop_width=$screen.Width; desktop_height=$screen.Height; detail_x=$detail.X; detail_y=$detail.Y; detail_width=$detail.Width; detail_height=$detail.Height } |
        ConvertTo-Json | Set-Content (Join-Path $out 'tray.json') -Encoding utf8
}
foreach ($variant in @(@{theme='dark'; lang='uk'}, @{theme='light'; lang='en'})) {
    $settings = "language=$($variant.lang)" + [Environment]::NewLine + "theme=$($variant.theme)" + [Environment]::NewLine + 'auto_update=false'
    [IO.File]::WriteAllText((Join-Path $programDir 'recycle-desk.ini'), $settings)
    $appProcess = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
    try {
        $window = [IntPtr]::Zero
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            Start-Sleep -Milliseconds 250
            $window = [CaptureWindow]::FindWindow('RecycleDesk.Main.v1', 'Recycle Desk')
            if ($window -ne [IntPtr]::Zero) { break }
            if ($appProcess.HasExited) { throw 'Application exited before capture' }
        }
        if ($window -eq [IntPtr]::Zero) { throw 'Application window not found' }
        $ownerPid = [uint32]0
        [void][CaptureWindow]::GetWindowThreadProcessId($window, [ref]$ownerPid)
        if ($ownerPid -ne $appProcess.Id) { throw 'Unexpected window owner' }
        [void][CaptureWindow]::ShowWindow($window, 5)
        [void][CaptureWindow]::MoveWindow($window, 20, 20, 1100, 780, $true)
        Start-Sleep -Seconds 6
        Save-AppWindow $window "main-$($variant.theme)-$($variant.lang)-v012.png"
        if ($variant.theme -eq 'dark') {
            Save-TrayDesktop $window
            [void][CaptureWindow]::MoveWindow($window, 20, 20, 1100, 780, $true)
            $search = [CaptureWindow]::GetDlgItem($window, 108)
            if ($search -eq [IntPtr]::Zero) { throw 'Search control not found' }
            [void][CaptureWindow]::SendMessage($search, 12, [IntPtr]::Zero, 'shapes')
            Start-Sleep -Seconds 2
            Save-AppWindow $window 'search-dark-uk-v012.png'
        }
    } finally {
        if (-not $appProcess.HasExited) {
            if ($window -ne [IntPtr]::Zero) { [void][CaptureWindow]::PostMessage($window, 273, [IntPtr]107, [IntPtr]::Zero) }
            if (-not $appProcess.WaitForExit(5000)) { $appProcess.Kill(); $appProcess.WaitForExit() }
        }
    }
}
$metadata = [ordered]@{
    application_version = [Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductVersion
    executable_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    captured_utc = [DateTime]::UtcNow.ToString('o')
    windows = (Get-CimInstance Win32_OperatingSystem).Caption
    fixtures = 'Six disposable demo files under C:\Recycle Desk Demo; no personal data.'
    capture = 'PrintWindow for app windows; CopyFromScreen for the real desktop and Explorer tray. Unmodified published application; no mock UI.'
}
$metadata | ConvertTo-Json | Set-Content (Join-Path $out 'capture.json') -Encoding utf8
