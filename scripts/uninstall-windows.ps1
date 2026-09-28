$ErrorActionPreference = "Stop"

$installRoot = $PSScriptRoot
$programExe = Join-Path $installRoot "md-previewer.exe"
$classesRoot = $env:MD_PREVIEWER_CLASSES_ROOT
if ([string]::IsNullOrWhiteSpace($classesRoot)) {
    $classesRoot = "HKCU:\Software\Classes"
}
$uninstallRoot = $env:MD_PREVIEWER_UNINSTALL_ROOT
if ([string]::IsNullOrWhiteSpace($uninstallRoot)) {
    $uninstallRoot = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MDPreviewer'
}
$startMenu = $env:MD_PREVIEWER_START_MENU_DIR
if ([string]::IsNullOrWhiteSpace($startMenu)) {
    $startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\MD Previewer"
}
$progid = "MDPreviewer.md"
$shellClsid = "{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}"
$extensions = @('.md', '.markdown', '.mdown', '.mkd', '.txt', '.json', '.toml', '.yaml', '.yml', '.log', '.env')

$runningInstalled = Get-Process -Name "md-previewer" -ErrorAction SilentlyContinue | Where-Object {
    try {
        [string]::Equals($_.Path, $programExe, [StringComparison]::OrdinalIgnoreCase)
    }
    catch {
        $false
    }
}
if ($runningInstalled) {
    throw "Close MD Previewer before uninstalling it."
}

Remove-Item -LiteralPath $startMenu -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $uninstallRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $classesRoot $progid) -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $classesRoot "Applications\md-previewer.exe") -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $classesRoot "*\shell\MDPreviewer") -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $classesRoot "CLSID\$shellClsid") -Recurse -Force -ErrorAction SilentlyContinue
foreach ($extension in $extensions) {
    $openWith = Join-Path $classesRoot "$extension\OpenWithProgids"
    Remove-ItemProperty -LiteralPath $openWith -Name $progid -ErrorAction SilentlyContinue
}
if ($classesRoot -eq "HKCU:\Software\Classes") {
    foreach ($extension in $extensions) {
        $openWith = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\$extension\OpenWithProgids"
        Remove-ItemProperty -LiteralPath $openWith -Name $progid -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath (Join-Path $env:LOCALAPPDATA "md-previewer\md-previewer-shell.dll") -Force -ErrorAction SilentlyContinue
}

$escapedInstallRoot = $installRoot.Replace("'", "''")
$cleanup = @"
for (`$attempt = 0; `$attempt -lt 40; `$attempt++) {
    Start-Sleep -Milliseconds 250
    Remove-Item -LiteralPath '$escapedInstallRoot' -Recurse -Force -ErrorAction SilentlyContinue
    if (-not (Test-Path -LiteralPath '$escapedInstallRoot')) {
        break
    }
}
"@
$encodedCleanup = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($cleanup))
$process = New-Object Diagnostics.ProcessStartInfo
$process.FileName = "powershell.exe"
$process.Arguments = "-NoProfile -WindowStyle Hidden -EncodedCommand $encodedCleanup"
$process.WorkingDirectory = $env:TEMP
$process.UseShellExecute = $false
[Diagnostics.Process]::Start($process) | Out-Null

Write-Host "MD Previewer was uninstalled. User settings were preserved."
