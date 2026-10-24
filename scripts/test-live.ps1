param([string]$Exe = 'target/release/recycle-desk.exe')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$program=Join-Path $root $Exe
$report=Join-Path $root 'test-output'
$process=Start-Process -FilePath $program -ArgumentList '--self-test',"`"$report`"" -WindowStyle Hidden -PassThru
if (-not $process.WaitForExit(30000)) { throw 'The integration test is still running; inspect its window before retrying.' }
Get-Content (Join-Path $report 'integration.txt') -ErrorAction SilentlyContinue
if ($process.ExitCode) {
    Get-Content (Join-Path $report 'checkpoint.txt') -ErrorAction SilentlyContinue
    throw "Integration check failed: $($process.ExitCode)"
}
