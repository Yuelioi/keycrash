param(
    [string]$ExePath
)

$ErrorActionPreference = 'Stop'
if (-not $ExePath) {
    $ExePath = Join-Path (& (Join-Path $PSScriptRoot 'get-target-root.ps1')) 'release\keycrash.exe'
}
$ExePath = (Resolve-Path -LiteralPath $ExePath).Path
if (-not ('KeyCrash.IconResources' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace KeyCrash {
    public static class IconResources {
        [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
        public static extern uint ExtractIconEx(string file, int index, IntPtr large, IntPtr small, uint count);
    }
}
'@
}
$count = [KeyCrash.IconResources]::ExtractIconEx($ExePath, -1, [IntPtr]::Zero, [IntPtr]::Zero, 0)
if ($count -eq 0 -or $count -eq [uint32]::MaxValue) {
    throw "Executable has no readable embedded icon: $ExePath"
}
Write-Output "PASS embedded icon: $ExePath ($count icon group)"
