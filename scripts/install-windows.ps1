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
    throw "Close the installed MD Previewer before updating it."
}

New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "md-previewer.exe") -Destination $programExe -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "uninstall-windows.ps1") -Destination $uninstaller -Force
# The uninstaller dot-sources this, so it has to sit next to it in the install dir
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "association-prune.ps1") -Destination (Join-Path $installRoot "association-prune.ps1") -Force
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
$fileExtsRoot = $env:MD_PREVIEWER_FILE_EXTS_ROOT
if ([string]::IsNullOrWhiteSpace($fileExtsRoot)) {
    $fileExtsRoot = $null
}
# Sweep legacy Open With entries before writing the fresh ones: older builds
# registered one Applications entry per exe file name and Windows keeps every
# name it ever used in FileExts\<ext>\OpenWithList
. (Join-Path $PSScriptRoot "association-prune.ps1")
Remove-LegacyPreviewerEntries -ClassesRoot $classesRoot -Extensions $extensions -FileExtsRoot $fileExtsRoot

New-Item -Path (Join-Path $progidRoot "DefaultIcon") -Force | Out-Null
New-Item -Path (Join-Path $progidRoot "shell\open\command") -Force | Out-Null
Set-Item -Path $progidRoot -Value "MD Previewer Markdown Document"
Set-Item -Path (Join-Path $progidRoot "DefaultIcon") -Value "`"$programExe`",0"
Set-Item -Path (Join-Path $progidRoot "shell\open\command") -Value "`"$programExe`" `"%1`""
foreach ($extension in $extensions) {
    $openWith = Join-Path $classesRoot "$extension\OpenWithProgids"
    New-Item -Path $openWith -Force | Out-Null
    New-ItemProperty -Path $openWith -Name $progid -Value '' -PropertyType String -Force | Out-Null
}
if ($classesRoot -eq "HKCU:\Software\Classes" -or -not [string]::IsNullOrWhiteSpace($fileExtsRoot)) {
    $feRoot = if ([string]::IsNullOrWhiteSpace($fileExtsRoot)) {
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts"
    }
    else {
        $fileExtsRoot
    }
    foreach ($extension in $extensions) {
        $openWith = "$feRoot\$extension\OpenWithProgids"
        New-Item -Path $openWith -Force | Out-Null
        New-ItemProperty -Path $openWith -Name $progid -Value '' -PropertyType String -Force | Out-Null
    }
}

$applicationRoot = Join-Path $classesRoot "Applications\md-previewer.exe"
New-Item -Path (Join-Path $applicationRoot "shell\open\command") -Force | Out-Null
New-Item -Path (Join-Path $applicationRoot "SupportedTypes") -Force | Out-Null
New-ItemProperty -Path $applicationRoot -Name 'FriendlyAppName' -Value 'MD Previewer' -PropertyType String -Force | Out-Null
Set-Item -Path (Join-Path $applicationRoot "shell\open\command") -Value "`"$programExe`" `"%1`""
foreach ($extension in $extensions) {
    New-ItemProperty -Path (Join-Path $applicationRoot "SupportedTypes") -Name $extension -Value '' -PropertyType String -Force | Out-Null
}

$shellDll = Join-Path $installRoot "md-previewer-shell.dll"
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "md-previewer-shell.dll") -Destination $shellDll -Force
$menuTitle = "Edit with MD Previewer"
if ([Globalization.CultureInfo]::CurrentUICulture.Name -like "zh*") {
    $menuTitle = "{0} MD Previewer {1}{2}" -f [char]0x4EE5, [char]0x7F16, [char]0x8F91
}
$shellVerb = Join-Path $classesRoot "*\shell\MDPreviewer"
New-Item -Path $shellVerb -Force | Out-Null
Set-Item -LiteralPath $shellVerb -Value $menuTitle
New-ItemProperty -LiteralPath $shellVerb -Name 'NeverDefault' -Value '' -PropertyType String -Force | Out-Null
New-ItemProperty -LiteralPath $shellVerb -Name 'Icon' -Value "`"$programExe`",0" -PropertyType String -Force | Out-Null
New-ItemProperty -LiteralPath $shellVerb -Name 'ExplorerCommandHandler' -Value $shellClsid -PropertyType String -Force | Out-Null
$clsidRoot = Join-Path $classesRoot "CLSID\$shellClsid"
New-Item -Path (Join-Path $clsidRoot "InProcServer32") -Force | Out-Null
Set-Item -LiteralPath $clsidRoot -Value "MD Previewer"
New-ItemProperty -LiteralPath $clsidRoot -Name 'Exe' -Value $programExe -PropertyType String -Force | Out-Null
New-ItemProperty -LiteralPath $clsidRoot -Name 'Title' -Value $menuTitle -PropertyType String -Force | Out-Null
Set-Item -LiteralPath (Join-Path $clsidRoot "InProcServer32") -Value $shellDll
New-ItemProperty -LiteralPath (Join-Path $clsidRoot "InProcServer32") -Name 'ThreadingModel' -Value 'Apartment' -PropertyType String -Force | Out-Null

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
