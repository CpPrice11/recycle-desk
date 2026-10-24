param([string]$PrivateKey = (Join-Path $env:LOCALAPPDATA 'RecycleDesk/release-signing/private.key'))
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    . "$PSScriptRoot/gnu-env.ps1"
    $run=Join-Path $root ('test-output/updater-'+[Guid]::NewGuid().ToString('N'))
    $install=Join-Path $run 'install'
    $package=Join-Path $run 'package'
    New-Item -ItemType Directory -Force $install,$package | Out-Null
    $env:RECYCLE_DESK_PROBE_VERSION='old'
    cargo +stable-x86_64-pc-windows-gnu build --release --locked --offline --example update_probe
    if($LASTEXITCODE){throw 'Old probe build failed'}
    Copy-Item 'target/release/examples/update_probe.exe' (Join-Path $install 'recycle-desk.exe')
    $env:RECYCLE_DESK_PROBE_VERSION='new'
    cargo +stable-x86_64-pc-windows-gnu build --release --locked --offline --example update_probe
    if($LASTEXITCODE){throw 'New probe build failed'}
    Copy-Item 'target/release/examples/update_probe.exe' (Join-Path $package 'recycle-desk.exe')
    Set-Content (Join-Path $package 'LICENSE') 'MIT probe'
    Set-Content (Join-Path $package 'README.md') 'Signed updater test'
    Set-Content (Join-Path $package 'THIRD_PARTY_NOTICES.txt') 'Probe license notices'
    $settings=Join-Path $install 'recycle-desk.ini'
    Set-Content $settings "language=uk`ntheme=dark`nauto_update=false"
    $before=(Get-FileHash $settings -Algorithm SHA256).Hash
    $unsigned=Join-Path $run 'unsigned.zip'
    $files=@('recycle-desk.exe','README.md','LICENSE','THIRD_PARTY_NOTICES.txt')|ForEach-Object{Join-Path $package $_}
    Compress-Archive -LiteralPath $files -DestinationPath $unsigned
    cargo +stable-x86_64-pc-windows-gnu build --release --locked --offline --example release_sign
    if($LASTEXITCODE){throw 'Signing tool build failed'}
    $signed=Join-Path $run 'RecycleDesk-0.2.0-windows-x64-update.zip'
    & 'target/release/examples/release_sign.exe' sign $PrivateKey $unsigned $signed
    if($LASTEXITCODE){throw 'Probe signing failed'}
    $report=Join-Path $run 'restarted.txt'
    $program=Join-Path $install 'recycle-desk.exe'
    $process=Start-Process $program -ArgumentList '--replace',"`"$signed`"",'0.2.0',"`"$report`"" -WindowStyle Hidden -PassThru
    if(-not $process.WaitForExit(30000)){throw 'Old probe did not exit'}
    if($process.ExitCode){throw "Probe failed: $($process.ExitCode)"}
    $deadline=(Get-Date).AddSeconds(15)
    while(-not(Test-Path $report)-and(Get-Date)-lt$deadline){Start-Sleep -Milliseconds 100}
    if(-not(Test-Path $report)-or(Get-Content $report -Raw).Trim()-ne'new'){throw 'Updated process did not restart correctly'}
    if((Get-FileHash $settings -Algorithm SHA256).Hash-ne$before){throw 'Settings changed during update'}
    $result='PASS: pinned signature -> staging -> running EXE replacement -> wait for old PID -> mutex reacquire -> new process; settings SHA-256 unchanged.'
    $result | Set-Content (Join-Path $root 'test-output/updater-integration.txt')
    Write-Output $result
} finally {
    $env:RECYCLE_DESK_PROBE_VERSION=$null
    Pop-Location
}
