$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $metadata = cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Cargo metadata failed' }
    $output = [Text.StringBuilder]::new()
    [void]$output.AppendLine("Third-party notices for Recycle Desk`nGenerated from Cargo.lock; includes build and test dependencies.`n")
    foreach ($package in ($metadata.packages | Where-Object source | Sort-Object name,version)) {
        [void]$output.AppendLine("========== $($package.name) $($package.version) ==========")
        [void]$output.AppendLine("License: $($package.license)`nSource: https://crates.io/crates/$($package.name)/$($package.version)")
        $directory = Split-Path $package.manifest_path -Parent
        $licenses = @(Get-ChildItem -LiteralPath $directory -File | Where-Object Name -Match '^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE|COPYRIGHT)([._-].*)?$')
        if ($package.license_file) { $licenses += Get-Item -LiteralPath (Join-Path $directory $package.license_file) }
        if (-not $licenses) { throw "No license text found: $($package.name)" }
        foreach ($license in ($licenses | Sort-Object FullName -Unique)) {
            [void]$output.AppendLine("--- $($license.Name) ---")
            [void]$output.AppendLine([IO.File]::ReadAllText($license.FullName))
        }
    }
    $palette = Join-Path $PSScriptRoot '../resources/catppuccin-LICENSE'
    [void]$output.AppendLine("========== Catppuccin palette ==========`nhttps://github.com/catppuccin/palette")
    [void]$output.AppendLine([IO.File]::ReadAllText($palette))
    [IO.File]::WriteAllText((Join-Path (Get-Location) 'THIRD_PARTY_NOTICES.txt'), $output.ToString(), [Text.UTF8Encoding]::new($false))
    Write-Output 'Updated THIRD_PARTY_NOTICES.txt'
} finally { Pop-Location }
