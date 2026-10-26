# Runs only on a disposable GitHub Windows runner. Never use the local recycle bin.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:CI_SCREENSHOT_CAPTURE -ne 'true') {
    throw 'Screenshot capture is restricted to the dedicated GitHub Actions workflow.'
}
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName Microsoft.VisualBasic
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CaptureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
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
function Save-DesktopImage([string]$name) {
    $screen = [Windows.Forms.Screen]::PrimaryScreen.Bounds
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
    $console = [IntPtr]::Zero
    while (($console = [CaptureWindow]::FindWindowEx([IntPtr]::Zero, $console, 'ConsoleWindowClass', $null)) -ne [IntPtr]::Zero) {
        [void][CaptureWindow]::ShowWindow($console, 0)
    }
    [void][CaptureWindow]::MoveWindow($window, 24, 24, 900, 640, $true)
    [void][CaptureWindow]::RedrawWindow($window, [IntPtr]::Zero, [IntPtr]::Zero, 0x185)
    Start-Sleep -Seconds 2
    Save-DesktopImage 'tray-diagnostic.png'
    $area = [Windows.Automation.AutomationElement]::FromHandle($notify)
    $elements = $area.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    $icon = $elements | Where-Object { $_.Current.Name -like 'Recycle Desk*' -and -not $_.Current.IsOffscreen } | Select-Object -First 1
    if (-not $icon) {
        $chevron = $elements | Where-Object { $_.Current.AutomationId -eq '1502' -or $_.Current.Name -match 'hidden icons|Notification Chevron' } | Select-Object -First 1
        if (-not $chevron) {
            $elements | ForEach-Object { Write-Output "Notification element: $($_.Current.Name) [$($_.Current.AutomationId)]" }
            throw 'Tray icon and overflow button were not found'
        }
        $buttonBounds = $chevron.Current.BoundingRectangle
        if ($buttonBounds.Width -lt 8 -or $buttonBounds.Height -lt 8 -or $chevron.Current.IsOffscreen) { throw 'Overflow button is not visible' }
        Write-Output "Overflow button: $($chevron.Current.Name), bounds=$buttonBounds, handle=$($chevron.Current.NativeWindowHandle)"
        $button = [CaptureWindow]::GetDlgItem($notify, 1502)
        if ($button -eq [IntPtr]::Zero) { throw 'Native overflow button not found' }
        [void][CaptureWindow]::PostMessage($button, 0xF5, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Seconds 1
        Save-DesktopImage 'overflow-diagnostic.png'
        $overflow = [CaptureWindow]::FindWindow('NotifyIconOverflowWindow', $null)
        if ($overflow -eq [IntPtr]::Zero) { throw 'Notification overflow did not open' }
        $area = [Windows.Automation.AutomationElement]::FromHandle($overflow)
        $elements = $area.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
        $icon = $elements | Where-Object { $_.Current.Name -like 'Recycle Desk*' -and -not $_.Current.IsOffscreen } | Select-Object -First 1
    }
    if (-not $icon) {
        $elements | ForEach-Object { Write-Output "Notification element: $($_.Current.Name) [$($_.Current.AutomationId)] offscreen=$($_.Current.IsOffscreen), bounds=$($_.Current.BoundingRectangle)" }
        Write-Output "Overflow native visibility: $([CaptureWindow]::FindWindow('NotifyIconOverflowWindow', $null))"
        throw 'Recycle Desk is not visible in the notification area'
    }
    $bounds = $icon.Current.BoundingRectangle
    if ($bounds.Width -lt 8 -or $bounds.Height -lt 8) { throw 'Invalid tray icon bounds' }
    [void][CaptureWindow]::SetCursorPos([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))
    Start-Sleep -Milliseconds 1800
    $screen = [Windows.Forms.Screen]::PrimaryScreen.Bounds
    Save-DesktopImage 'tray-desktop-uk-v012.png'
    [ordered]@{ name=$icon.Current.Name; x=$bounds.X; y=$bounds.Y; width=$bounds.Width; height=$bounds.Height; desktop_width=$screen.Width; desktop_height=$screen.Height } |
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
