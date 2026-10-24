param([string]$Target = 'x86_64-pc-windows-msvc', [string]$Iscc = 'iscc.exe')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $versionLine = Select-String -LiteralPath 'Cargo.toml' -Pattern '^version = "([0-9.]+)"'
    $version = $versionLine.Matches[0].Groups[1].Value
    $exe = if ($Target -eq 'native') { Join-Path $root 'target/release/recycle-desk.exe' } else { Join-Path $root "target/$Target/release/recycle-desk.exe" }
    if (-not (Test-Path $exe)) { throw "Build missing: $exe" }
    $stage = Join-Path $root "dist/RecycleDesk-$version-windows-x64"
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'recycle-desk.exe')
    Copy-Item -LiteralPath 'LICENSE','README.md','THIRD_PARTY_NOTICES.txt' -Destination $stage
    New-Item -ItemType File -Force (Join-Path $stage 'portable.flag') | Out-Null
    $zip = "$stage.zip"
    Compress-Archive -LiteralPath $stage -DestinationPath $zip -Force
    & $Iscc "/DAppVersion=$version" "/DSourceExe=$exe" 'installer/RecycleDesk.iss'
    if ($LASTEXITCODE) { throw 'Installer build failed' }
    Get-ChildItem 'dist' -File | Where-Object Extension -In '.zip','.exe' | ForEach-Object {
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $($_.Name)"
    } | Set-Content -LiteralPath 'dist/SHA256SUMS.txt' -Encoding ascii
} finally { Pop-Location }
