param([ValidateSet('debug', 'release')][string]$Profile = 'release')

$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$targetRoot = & (Join-Path $PSScriptRoot 'get-target-root.ps1')
$releaseRoot = Join-Path $targetRoot $Profile
[string[]]$profileArgs = @(if ($Profile -eq 'release') { '--release' })
$x86Target = 'i686-pc-windows-msvc'

cargo build --locked @profileArgs --workspace --target-dir $targetRoot --manifest-path (Join-Path $projectRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    throw "x64 $Profile build failed with exit code $LASTEXITCODE"
}

$installedTargets = rustup target list --installed
if ($installedTargets -notcontains $x86Target) {
    rustup target add $x86Target
}

cargo build --locked @profileArgs --target $x86Target --target-dir $targetRoot `
    -p keycrash-owner-hook `
    -p keycrash-owner-probe `
    --manifest-path (Join-Path $projectRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    throw "x86 $Profile build failed with exit code $LASTEXITCODE"
}

$x86Release = Join-Path $targetRoot "$x86Target\$Profile"
$x86Bundle = Join-Path $releaseRoot 'owner-x86'
New-Item -ItemType Directory -Path $x86Bundle -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $x86Release 'keycrash-owner-probe.exe') `
    -Destination (Join-Path $x86Bundle 'keycrash-owner-probe.exe') -Force
Copy-Item -LiteralPath (Join-Path $x86Release 'keycrash_owner_hook.dll') `
    -Destination (Join-Path $x86Bundle 'keycrash_owner_hook.dll') -Force

# Distinct DLL names are required for simultaneous x64/x86 global message hooks.
Copy-Item -LiteralPath (Join-Path $releaseRoot 'keycrash_owner_hook.dll') `
    -Destination (Join-Path $releaseRoot 'keycrash_trace64_v3.dll') -Force
Copy-Item -LiteralPath (Join-Path $x86Release 'keycrash_owner_hook.dll') `
    -Destination (Join-Path $x86Bundle 'keycrash_trace32_v3.dll') -Force

if ($Profile -eq 'release') {
    & (Join-Path $PSScriptRoot 'test-exe-icon.ps1') -ExePath (Join-Path $releaseRoot 'keycrash.exe')
}
Write-Output "Windows $Profile bundle: $releaseRoot"
