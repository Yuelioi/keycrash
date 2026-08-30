$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$releaseRoot = Join-Path $projectRoot 'target\release'
$x86Target = 'i686-pc-windows-msvc'

cargo build --release --workspace --manifest-path (Join-Path $projectRoot 'Cargo.toml')

$installedTargets = rustup target list --installed
if ($installedTargets -notcontains $x86Target) {
    rustup target add $x86Target
}

cargo build --release --target $x86Target `
    -p keycrash-owner-hook `
    -p keycrash-owner-probe `
    --manifest-path (Join-Path $projectRoot 'Cargo.toml')

$x86Release = Join-Path $projectRoot "target\$x86Target\release"
$x86Bundle = Join-Path $releaseRoot 'owner-x86'
New-Item -ItemType Directory -Path $x86Bundle -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $x86Release 'keycrash-owner-probe.exe') `
    -Destination (Join-Path $x86Bundle 'keycrash-owner-probe.exe') -Force
Copy-Item -LiteralPath (Join-Path $x86Release 'keycrash_owner_hook.dll') `
    -Destination (Join-Path $x86Bundle 'keycrash_owner_hook.dll') -Force

Write-Output "Release bundle: $releaseRoot"
