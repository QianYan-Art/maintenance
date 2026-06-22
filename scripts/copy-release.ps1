$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$source = Join-Path $root "target\release\maintenance.exe"
$destDir = Join-Path $root "skill\doc-maintenance\bin"
$dest = Join-Path $destDir "maintenance.exe"

if (-not (Test-Path -LiteralPath $source)) {
    throw "Release binary not found. Run `cargo build --release` first."
}

New-Item -ItemType Directory -Force -Path $destDir | Out-Null
Copy-Item -LiteralPath $source -Destination $dest -Force
Write-Host "Copied $source to $dest"
