# Local build environment; release CI uses the standard MSVC toolchain.
$compiler = Join-Path (Split-Path $PSScriptRoot -Parent) '.tools/llvm-mingw-20261020-msvcrt-x86_64/bin'
if (Test-Path $compiler) { $env:PATH = "$compiler;$env:PATH" }
$sysroot = rustc +stable-x86_64-pc-windows-gnu --print sysroot
if ($LASTEXITCODE) { throw 'GNU Rust toolchain is not installed' }
$linker = Join-Path $sysroot 'lib/rustlib/x86_64-pc-windows-gnu/bin/rust-lld.exe'
$env:CARGO_ENCODED_RUSTFLAGS = @('-Clink-self-contained=yes', "-Clinker=$linker", '-Clinker-flavor=ld.lld') -join [char]31
$env:CARGO_INCREMENTAL = '0'
