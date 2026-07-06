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

# Skills roots may alias the same physical directory (junction/symlink);
# resolve links and sync each physical directory only once.
$syncedRoots = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($target in $targets) {
    $physicalRoot = Split-Path -Parent $target
    $item = Get-Item -LiteralPath $physicalRoot -ErrorAction SilentlyContinue
    while ($null -ne $item -and $item.LinkType -and $item.LinkTarget) {
        $physicalRoot = $item.LinkTarget
        $item = Get-Item -LiteralPath $physicalRoot -ErrorAction SilentlyContinue
    }
    if (-not $syncedRoots.Add($physicalRoot)) {
        Write-Host "Skipped $target (same physical skills directory already synced)"
        continue
    }
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    Get-ChildItem -LiteralPath $sourceDir | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $target -Recurse -Force
    }
    Write-Host "Synced $sourceDir to $target"
}
