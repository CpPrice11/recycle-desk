$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$archive = Join-Path $root '.tools/innosetup-7.1.0-x64.exe'
$destination = Join-Path $root '.tools/inno'
New-Item -ItemType Directory -Force (Join-Path $root '.tools') | Out-Null
if (-not (Test-Path $archive)) {
    Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_0/innosetup-7.1.0-x64.exe' -OutFile $archive
}
if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne '0362A383ED217D4C4239B5933866DD96D3EB2102737DA92F80F6057A4B40DF2F') { throw 'Inno Setup checksum mismatch' }
if (-not (Test-Path (Join-Path $destination 'ISCC.exe'))) {
    $process = Start-Process -FilePath $archive -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-','/PORTABLE=1',"/DIR=`"$destination`"" -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode) { throw "Inno Setup portable extraction failed: $($process.ExitCode)" }
}
Write-Output (Join-Path $destination 'ISCC.exe')
