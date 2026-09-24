$ErrorActionPreference = "Stop"

# Opening a large document on a cold start must not kill the app.
#
# The preview page is handed to WebView2 through NavigateToString, which is capped at 2MiB:
# if the startup HTML carries the document body, the webview cannot be created at all and the
# process dies before the first paint. The document therefore has to be pushed through
# __setContent after the page is ready.
#
# This script only asserts what is visible from the outside: the process stays alive, shows a
# window, records the tab and writes no crash entry. Whether the body actually painted cannot be
# observed from here (wry overrides WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, so CDP cannot attach);
# tests::startup_page_stays_small_even_for_a_huge_document plus manual acceptance cover that part.
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$exe = (Resolve-Path -LiteralPath (Join-Path $root "target\release\md-previewer.exe")).Path
$config = Join-Path ([IO.Path]::GetTempPath()) ("md-previewer-large-document-" + [guid]::NewGuid())
[IO.Directory]::CreateDirectory($config) | Out-Null
[IO.File]::WriteAllText((Join-Path $config ".md-previewer-registered"), "")
$doc = Join-Path $config "large.md"
$process = $null

# Roughly 4MB of body text: the old first-paint HTML for this file was around 8MB, far past the cap
$chunk = "# Section`n`nParagraph with **bold**, ``code`` and a [link](https://example.com).`n`n"
$text = [Text.StringBuilder]::new()
1..60000 | ForEach-Object { [void]$text.Append($chunk) }
[IO.File]::WriteAllText($doc, $text.ToString(), [Text.UTF8Encoding]::new($false))
$size = (Get-Item -LiteralPath $doc).Length
if ($size -lt 2mb) {
    throw "sample document is too small to cover the 2MiB startup limit: $size bytes"
}

try {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $exe
    $info.Arguments = "`"$doc`""
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $root
    $info.EnvironmentVariables["MD_PREVIEWER_CONFIG_DIR"] = $config
    $process = [Diagnostics.Process]::Start($info)

    # A cold runner has to boot WebView2 first, so give the window a generous deadline
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ($true) {
        if ($process.HasExited) {
            $log = Join-Path $config "app.log"
            $detail = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw } else { "(no log)" }
            throw "app exited with $($process.ExitCode) while opening a $([int]($size / 1mb))MB document: $detail"
        }
        if ($process.MainWindowHandle -ne 0) {
            break
        }
        if ([DateTime]::UtcNow -gt $deadline) {
            throw "app did not show a window within 30s while opening a $([int]($size / 1mb))MB document"
        }
        Start-Sleep -Milliseconds 200
    }

    # session.json is written after the window is up and before the first document is pushed, so
    # seeing the tab here proves the crash did not happen after the window appeared
    $sessionPath = Join-Path $config "session.json"
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ($true) {
        if (Test-Path -LiteralPath $sessionPath) {
            $session = Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
            if ($session.tabs.Count -eq 1 -and $session.tabs[0] -eq $doc) {
                break
            }
        }
        if ($process.HasExited) {
            throw "app exited with $($process.ExitCode) before the document showed up in session.json"
        }
        if ([DateTime]::UtcNow -gt $deadline) {
            throw "session.json never listed the large document"
        }
        Start-Sleep -Milliseconds 200
    }

    $log = Join-Path $config "app.log"
    if (Test-Path -LiteralPath $log) {
        $content = Get-Content -LiteralPath $log -Raw
        if ($content -match "CRASH") {
            throw "app logged a crash while opening a $([int]($size / 1mb))MB document: $content"
        }
    }

    Write-Output "WINDOWS_LARGE_DOCUMENT_STARTUP_OK"
    Write-Output "DOCUMENT_BYTES=$size"
}
finally {
    if ($process -and -not $process.HasExited) {
        $process.Kill()
        $process.WaitForExit(5000) | Out-Null
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
