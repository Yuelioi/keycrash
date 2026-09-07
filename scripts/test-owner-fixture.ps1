param(
    [string]$ReleaseRoot
)

$ErrorActionPreference = 'Stop'

if (-not $ReleaseRoot) {
    $ReleaseRoot = Join-Path (& (Join-Path $PSScriptRoot 'get-target-root.ps1')) 'release'
}

$fixturePath = Join-Path $ReleaseRoot 'keycrash-hotkey-fixture.exe'
$probePath = Join-Path $ReleaseRoot 'keycrash-owner-probe.exe'

foreach ($path in @($fixturePath, $probePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Missing test artifact: $path"
    }
}

function Invoke-OwnerFixture {
    param(
        [uint32]$RegisteredModifiers,
        [uint32]$ExpectedModifiers,
        [uint32]$VirtualKey,
        [string]$Name
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $fixturePath
    $startInfo.ArgumentList.Add($RegisteredModifiers.ToString())
    $startInfo.ArgumentList.Add($VirtualKey.ToString())
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.CreateNoWindow = $true

    $fixture = [System.Diagnostics.Process]::new()
    $fixture.StartInfo = $startInfo
    if (-not $fixture.Start()) {
        throw "$Name fixture did not start"
    }

    try {
        $ready = $fixture.StandardOutput.ReadLineAsync()
        if (-not $ready.Wait(3000)) {
            throw "$Name fixture did not become ready"
        }
        if ($ready.Result -notmatch '^READY\t(?<pid>\d+)\t(?<tid>\d+)$') {
            $stderr = $fixture.StandardError.ReadToEnd()
            throw "$Name fixture failed before ready: $($ready.Result) $stderr"
        }
        $fixturePid = [uint32]$Matches.pid

        $probeOutput = & $probePath $ExpectedModifiers $VirtualKey
        if ($LASTEXITCODE -ne 0) {
            throw "$Name probe exited with $LASTEXITCODE`: $probeOutput"
        }
        if ($probeOutput -notmatch '^FOUND\t(?<pid>\d+)\t') {
            $fixtureState = if ($fixture.WaitForExit(3000)) {
                'exited'
            }
            else {
                $fixture.Kill($true)
                $fixture.WaitForExit()
                'still waiting for WM_HOTKEY'
            }
            $messageOutput = $fixture.StandardOutput.ReadToEnd().Trim()
            $fixtureError = $fixture.StandardError.ReadToEnd().Trim()
            throw "$Name expected FOUND, received: $probeOutput; fixture $fixtureState`: $messageOutput $fixtureError"
        }
        if ([uint32]$Matches.pid -ne $fixturePid) {
            throw "$Name expected PID $fixturePid, received: $probeOutput"
        }

        if (-not $fixture.WaitForExit(3000)) {
            throw "$Name fixture did not exit after the probe"
        }
        Write-Output "PASS $Name $probeOutput"
    }
    finally {
        if (-not $fixture.HasExited) {
            $fixture.Kill($true)
            $fixture.WaitForExit()
        }
        $fixture.Dispose()
    }
}

# F13-F15 avoid user-configured shortcuts while preserving the modifier shapes.
Invoke-OwnerFixture -RegisteredModifiers 2 -ExpectedModifiers 2 -VirtualKey 124 -Name 'ctrl-key'
Invoke-OwnerFixture -RegisteredModifiers 0 -ExpectedModifiers 0 -VirtualKey 125 -Name 'bare-key'
Invoke-OwnerFixture -RegisteredModifiers 0x4000 -ExpectedModifiers 0 -VirtualKey 126 -Name 'bare-key-norepeat'
