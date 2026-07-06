$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$sourceDir = Join-Path $root "skill\doc-maintenance"
$releaseBinary = Join-Path $root "target\release\maintenance.exe"
$skillBinaryDir = Join-Path $sourceDir "bin"
$skillBinary = Join-Path $skillBinaryDir "maintenance.exe"
$homeDir = [Environment]::GetFolderPath("UserProfile")
$targets = @(
    (Join-Path $homeDir ".codex\skills\doc-maintenance"),
    (Join-Path $homeDir ".claude\skills\doc-maintenance")
)

Push-Location $root
try {
    cargo build --release
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $releaseBinary)) {
    throw "Release binary not found after cargo build --release."
}

New-Item -ItemType Directory -Force -Path $skillBinaryDir | Out-Null
Copy-Item -LiteralPath $releaseBinary -Destination $skillBinary -Force

foreach ($target in $targets) {
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    Get-ChildItem -LiteralPath $sourceDir | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $target -Recurse -Force
    }
    Write-Host "Synced $sourceDir to $target"
}
