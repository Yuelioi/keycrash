$ErrorActionPreference = 'Stop'

# Resolve the global Cargo target directory, then isolate this project's output.
$projectRoot = Split-Path -Parent $PSScriptRoot
$metadataJson = cargo metadata --no-deps --format-version 1 --manifest-path (Join-Path $projectRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    throw "Cargo metadata failed with exit code $LASTEXITCODE"
}
$baseTargetRoot = ($metadataJson | ConvertFrom-Json).target_directory
Join-Path $baseTargetRoot 'keycrash'
