param([string]$Target = 'x86_64-pc-windows-msvc', [switch]$Gnu)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
$previousFlags = $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS
try {
    if ($Gnu) {
        $Target = 'x86_64-pc-windows-gnu'
        . "$PSScriptRoot/gnu-env.ps1"
        cargo +stable-x86_64-pc-windows-gnu build --release --locked
    } else {
        if ($Target -eq 'x86_64-pc-windows-msvc') {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = "$previousFlags -Ctarget-feature=+crt-static".Trim()
        }
        cargo build --release --locked --target $Target --bin recycle-desk
    }
    if ($LASTEXITCODE) { throw 'Rust build failed' }
} finally {
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = $previousFlags
    Pop-Location
}
