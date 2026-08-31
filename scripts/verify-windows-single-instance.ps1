$ErrorActionPreference = "Stop"

$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$exe = (Resolve-Path -LiteralPath (Join-Path $root "target\release\md-previewer.exe")).Path
$config = Join-Path ([IO.Path]::GetTempPath()) ("md-previewer-single-instance-" + [guid]::NewGuid())
[IO.Directory]::CreateDirectory($config) | Out-Null
[IO.File]::WriteAllText((Join-Path $config ".md-previewer-registered"), "")
$firstDoc = Join-Path $config "first.md"
$secondDoc = Join-Path $config "second.md"
[IO.File]::WriteAllText($firstDoc, "# first", [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($secondDoc, "# second", [Text.UTF8Encoding]::new($false))
$primary = $null
$replacement = $null

function Start-Previewer([string]$document) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $exe
    $info.Arguments = if ($document) { "`"$document`"" } else { "" }
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $root
    $info.EnvironmentVariables["MD_PREVIEWER_CONFIG_DIR"] = $config
    return [Diagnostics.Process]::Start($info)
}

function Wait-For([scriptblock]$condition, [string]$message, [int]$seconds = 10) {
    $deadline = [DateTime]::UtcNow.AddSeconds($seconds)
    do {
        if (& $condition) {
            return
        }
        if ([DateTime]::UtcNow -gt $deadline) {
            throw $message
        }
        Start-Sleep -Milliseconds 100
    } while ($true)
}

function Wait-Forwarded([Diagnostics.Process]$process) {
    if (-not $process.WaitForExit(5000)) {
        $process.Kill()
        throw "secondary instance did not exit after forwarding"
    }
    if ($process.ExitCode -ne 0) {
        throw "secondary instance exited with $($process.ExitCode)"
    }
}

function Read-Session {
    $sessionPath = Join-Path $config "session.json"
    if (-not (Test-Path -LiteralPath $sessionPath)) {
        return $null
    }
    try {
        return Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
    }
    catch {
        return $null
    }
}

try {
    $primary = Start-Previewer $firstDoc
    $lockPath = Join-Path $config "instance.lock"
    Wait-For { Test-Path -LiteralPath $lockPath } "primary instance did not create lock" 15

    $secondary = Start-Previewer $secondDoc
    Wait-Forwarded $secondary
    Wait-For {
        $session = Read-Session
        $session -and $session.tabs.Count -eq 2 -and $session.active -eq 1
    } "primary session did not receive the second document"

    $duplicate = Start-Previewer $secondDoc
    Wait-Forwarded $duplicate
    Start-Sleep -Milliseconds 300
    $session = Read-Session
    if (-not $session -or $session.tabs.Count -ne 2 -or $session.active -ne 1) {
        throw "opening the same document created a duplicate tab"
    }
    $names = @($session.tabs | ForEach-Object { [IO.Path]::GetFileName($_) })
    if ($names[0] -ne "first.md" -or $names[1] -ne "second.md") {
        throw "unexpected tab order: $($names -join ", ")"
    }

    $primary.Kill()
    $primary.WaitForExit(5000) | Out-Null
    $primary = $null

    $replacement = Start-Previewer $firstDoc
    Wait-For {
        if ($replacement.HasExited) {
            return $false
        }
        (Get-Content -LiteralPath $lockPath -Raw -ErrorAction SilentlyContinue).Trim() -eq [string]$replacement.Id
    } "a new primary could not recover after the old process exited"

    Write-Output "WINDOWS_SINGLE_INSTANCE_OK"
    Write-Output "TABS=$($names -join " | ")"
}
finally {
    foreach ($process in @($primary, $replacement)) {
        if ($process -and -not $process.HasExited) {
            $process.Kill()
            $process.WaitForExit(5000) | Out-Null
        }
    }
    if (Test-Path -LiteralPath $config) {
        $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            try {
                Remove-Item -LiteralPath $config -Recurse -Force -ErrorAction Stop
                break
            }
            catch {
                if ([DateTime]::UtcNow -gt $cleanupDeadline) {
                    Write-Warning "Could not remove temporary WebView2 cache: $config"
                    break
                }
                Start-Sleep -Milliseconds 200
            }
        } while ($true)
    }
}
