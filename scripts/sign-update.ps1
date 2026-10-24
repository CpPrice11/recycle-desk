param(
    [Parameter(Mandatory)][string]$PrivateKey,
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$Signer = 'target/release/examples/release_sign.exe',
    [string]$ExePath
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $match=Select-String -LiteralPath 'Cargo.toml' -Pattern '^version = "([0-9.]+)"'
    $version=$match.Matches[0].Groups[1].Value
    $exe=if($ExePath){$ExePath}elseif($Target -eq 'native'){'target/release/recycle-desk.exe'}else{"target/$Target/release/recycle-desk.exe"}
    if(-not(Test-Path $exe)){throw 'Application build is missing'}
    if(-not(Test-Path $Signer)){throw 'Build the release_sign example first'}
    New-Item -ItemType Directory -Force 'dist','.tools/update' | Out-Null
    $unsigned=Join-Path $root '.tools/update/unsigned.zip'
    $stage=Join-Path $root '.tools/update/files'
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'recycle-desk.exe')
    Copy-Item -LiteralPath 'LICENSE','README.md','THIRD_PARTY_NOTICES.txt' -Destination $stage
    $files=@('recycle-desk.exe','LICENSE','README.md','THIRD_PARTY_NOTICES.txt') | ForEach-Object {Join-Path $stage $_}
    Compress-Archive -LiteralPath $files -DestinationPath $unsigned -Force
    $signed=Join-Path $root "dist/RecycleDesk-$version-windows-x64-update.zip"
    & (Join-Path $root $Signer) sign $PrivateKey $unsigned $signed
    if($LASTEXITCODE){throw 'Update archive signing failed'}
    $hash=(Get-FileHash -LiteralPath $signed -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $(Split-Path $signed -Leaf)" | Add-Content -LiteralPath 'dist/SHA256SUMS.txt' -Encoding ascii
    Write-Output "Signed update: $signed"
} finally {Pop-Location}
