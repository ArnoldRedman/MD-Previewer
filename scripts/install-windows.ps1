$ErrorActionPreference = "Stop"

$installRoot = $env:MD_PREVIEWER_INSTALL_DIR
if ([string]::IsNullOrWhiteSpace($installRoot)) {
    $installRoot = Join-Path $env:LOCALAPPDATA "Programs\MD Previewer"
}
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
$programExe = Join-Path $installRoot "md-previewer.exe"
$uninstaller = Join-Path $installRoot "uninstall-windows.ps1"
$uninstallCmd = Join-Path $installRoot "uninstall.cmd"
$progid = "MDPreviewer.md"

$runningInstalled = Get-Process -Name "md-previewer" -ErrorAction SilentlyContinue | Where-Object {
    try {
        [string]::Equals($_.Path, $programExe, [StringComparison]::OrdinalIgnoreCase)
    }
    catch {
        $false
    }
}
if ($runningInstalled) {
    throw "Close the installed MD Previewer before updating it."
}

New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "md-previewer.exe") -Destination $programExe -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "uninstall-windows.ps1") -Destination $uninstaller -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "NOTICE") -Destination (Join-Path $installRoot "NOTICE") -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "LICENSE") -Destination (Join-Path $installRoot "LICENSE") -Force

$uninstallCmdBody = "@echo off`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"%~dp0uninstall-windows.ps1`"`r`n"
[IO.File]::WriteAllText($uninstallCmd, $uninstallCmdBody, [Text.Encoding]::ASCII)

$shortcutPath = Join-Path $startMenu "MD Previewer.lnk"
$uninstallShortcutPath = Join-Path $startMenu "Uninstall MD Previewer.lnk"
New-Item -ItemType Directory -Path $startMenu -Force | Out-Null
$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $programExe
$shortcut.WorkingDirectory = $installRoot
$shortcut.IconLocation = "$programExe,0"
$shortcut.Description = "MD Previewer Markdown reader"
$shortcut.Save()
$uninstallShortcut = $wsh.CreateShortcut($uninstallShortcutPath)
$uninstallShortcut.TargetPath = $uninstallCmd
$uninstallShortcut.WorkingDirectory = $installRoot
$uninstallShortcut.Description = "Uninstall MD Previewer"
$uninstallShortcut.Save()

$progidRoot = Join-Path $classesRoot $progid
New-Item -Path (Join-Path $progidRoot "DefaultIcon") -Force | Out-Null
New-Item -Path (Join-Path $progidRoot "shell\open\command") -Force | Out-Null
Set-Item -Path $progidRoot -Value "MD Previewer Markdown Document"
Set-Item -Path (Join-Path $progidRoot "DefaultIcon") -Value "`"$programExe`",0"
Set-Item -Path (Join-Path $progidRoot "shell\open\command") -Value "`"$programExe`" `"%1`""
foreach ($extension in @('.md', '.markdown', '.mdown', '.mkd', '.txt')) {
    $openWith = Join-Path $classesRoot "$extension\OpenWithProgids"
    New-Item -Path $openWith -Force | Out-Null
    New-ItemProperty -Path $openWith -Name $progid -Value '' -PropertyType String -Force | Out-Null
}

$applicationRoot = Join-Path $classesRoot "Applications\md-previewer.exe"
New-Item -Path (Join-Path $applicationRoot "shell\open\command") -Force | Out-Null
New-ItemProperty -Path $applicationRoot -Name 'FriendlyAppName' -Value 'MD Previewer' -PropertyType String -Force | Out-Null
Set-Item -Path (Join-Path $applicationRoot "shell\open\command") -Value "`"$programExe`" `"%1`""

$version = (Get-Item -LiteralPath $programExe).VersionInfo.ProductVersion
New-Item -Path $uninstallRoot -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'DisplayName' -Value 'MD Previewer' -PropertyType String -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'DisplayVersion' -Value $version -PropertyType String -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'Publisher' -Value 'ArnoldRedman' -PropertyType String -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'InstallLocation' -Value $installRoot -PropertyType String -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'DisplayIcon' -Value "$programExe,0" -PropertyType String -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'UninstallString' -Value "`"$uninstallCmd`"" -PropertyType String -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'NoModify' -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $uninstallRoot -Name 'NoRepair' -Value 1 -PropertyType DWord -Force | Out-Null

Write-Host "Installed MD Previewer to $installRoot"
