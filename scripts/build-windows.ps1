param(
    [switch]$SkipInstallerTest
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location -LiteralPath $root
$cargo = (Get-Command cargo.exe -ErrorAction Stop).Source
$versionMatch = [regex]::Match(
    (Get-Content -LiteralPath (Join-Path $root "Cargo.toml") -Raw),
    '(?m)^version\s*=\s*"([^"]+)"'
)
if (-not $versionMatch.Success) {
    throw "Could not read Cargo.toml version"
}
$version = $versionMatch.Groups[1].Value

$dist = Join-Path $root "dist"
$payload = Join-Path $dist "payload"
$releaseExe = Join-Path $root "target\release\md-previewer.exe"
$portableExe = Join-Path $dist "MD-Previewer-windows-x64.exe"
$installer = Join-Path $dist "MD-Previewer-Setup.exe"
$testInstall = Join-Path $dist "installer-test"
$testId = [guid]::NewGuid().ToString("N")
$testRegistryRoot = "HKCU:\Software\MDPreviewerBuildTest\$testId"
$testClassesRoot = Join-Path $testRegistryRoot "Classes"
$testUninstallRoot = Join-Path $testRegistryRoot "Uninstall"
$testStartMenu = Join-Path $dist "start-menu-test"

function Invoke-Cargo([string[]]$Arguments) {
    & $cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Quote-ProcessArgument([string]$Value) {
    return '"' + $Value.Replace('"', '\"') + '"'
}

function Start-WithEnvironment(
    [string]$FilePath,
    [string]$Arguments,
    [hashtable]$Environment
) {
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $FilePath
    $info.Arguments = $Arguments
    $info.WorkingDirectory = $root
    $info.UseShellExecute = $false
    foreach ($name in $Environment.Keys) {
        $info.EnvironmentVariables[$name] = [string]$Environment[$name]
    }
    return [Diagnostics.Process]::Start($info)
}

function Wait-ForExit([Diagnostics.Process]$Process, [string]$Description) {
    if (-not $Process.WaitForExit(120000)) {
        $Process.Kill()
        throw "$Description timed out"
    }
    if ($Process.ExitCode -ne 0) {
        throw "$Description failed with exit code $($Process.ExitCode)"
    }
}

if (-not $dist.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Output directory is outside the repository: $dist"
}

Write-Host "[build] Running Rust tests"
Invoke-Cargo @("test", "--locked")

Write-Host "[build] Building release executable"
Invoke-Cargo @("build", "--release", "--locked")
if (-not (Test-Path -LiteralPath $releaseExe)) {
    throw "Release executable was not created: $releaseExe"
}

New-Item -ItemType Directory -Path $dist -Force | Out-Null
foreach ($file in @($portableExe, $installer, (Join-Path $dist "SHA256SUMS.txt"))) {
    if (Test-Path -LiteralPath $file) {
        Remove-Item -LiteralPath $file -Force
    }
}
if (Test-Path -LiteralPath $payload) {
    Remove-Item -LiteralPath $payload -Recurse -Force
}
New-Item -ItemType Directory -Path $payload -Force | Out-Null
Copy-Item -LiteralPath $releaseExe -Destination $portableExe
Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $payload "md-previewer.exe")
foreach ($file in @("LICENSE", "NOTICE", "install-windows.cmd", "install-windows.ps1", "uninstall-windows.ps1")) {
    $source = if ($file -in @("LICENSE", "NOTICE")) {
        Join-Path $root $file
    }
    else {
        Join-Path $root "scripts\$file"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $payload $file)
}

$sed = Join-Path $dist "installer.sed"
$sedContent = @"
[Version]
Class=IEXPRESS
SEDVersion=3
[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=1
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=1
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=%InstallPrompt%
DisplayLicense=%DisplayLicense%
FinishMessage=%FinishMessage%
TargetName=%TargetName%
FriendlyName=%FriendlyName%
AppLaunched=%AppLaunched%
PostInstallCmd=%PostInstallCmd%
AdminQuietInstCmd=%AdminQuietInstCmd%
UserQuietInstCmd=%UserQuietInstCmd%
SourceFiles=SourceFiles
[Strings]
InstallPrompt=Install MD Previewer for the current user?
DisplayLicense=
FinishMessage=MD Previewer was installed for the current user.
TargetName=$installer
FriendlyName=MD Previewer $version Setup
AppLaunched=install-windows.cmd
PostInstallCmd=<None>
AdminQuietInstCmd=install-windows.cmd quiet
UserQuietInstCmd=install-windows.cmd quiet
FILE0=md-previewer.exe
FILE1=LICENSE
FILE2=NOTICE
FILE3=install-windows.cmd
FILE4=install-windows.ps1
FILE5=uninstall-windows.ps1
[SourceFiles]
SourceFiles0=$payload\
[SourceFiles0]
%FILE0%=
%FILE1%=
%FILE2%=
%FILE3%=
%FILE4%=
%FILE5%=
"@
[IO.File]::WriteAllText($sed, $sedContent, [Text.Encoding]::Default)

$iexpress = Join-Path $env:SystemRoot "System32\iexpress.exe"
if (-not (Test-Path -LiteralPath $iexpress)) {
    throw "Windows IExpress was not found: $iexpress"
}
Write-Host "[build] Creating IExpress installer"
$iexpressProcess = Start-Process -FilePath $iexpress -ArgumentList @("/N", "/Q", $sed) -Wait -PassThru
if ($iexpressProcess.ExitCode -ne 0) {
    throw "IExpress failed with exit code $($iexpressProcess.ExitCode)"
}
if (-not (Test-Path -LiteralPath $installer)) {
    throw "Installer was not created: $installer"
}

if (-not $SkipInstallerTest) {
    Write-Host "[build] Testing silent install and uninstall"
    $testEnvironment = @{
        MD_PREVIEWER_INSTALL_DIR = $testInstall
        MD_PREVIEWER_CLASSES_ROOT = $testClassesRoot
        MD_PREVIEWER_UNINSTALL_ROOT = $testUninstallRoot
        MD_PREVIEWER_START_MENU_DIR = $testStartMenu
        MD_PREVIEWER_INSTALL_QUIET = "1"
    }
    $installProcess = Start-WithEnvironment $installer "/Q" $testEnvironment
    Wait-ForExit $installProcess "installer test"
    $installedExe = Join-Path $testInstall "md-previewer.exe"
    if (-not (Test-Path -LiteralPath $installedExe)) {
        throw "Installer test did not create $installedExe"
    }
    if ((Get-FileHash -LiteralPath $installedExe -Algorithm SHA256).Hash -ne
        (Get-FileHash -LiteralPath $releaseExe -Algorithm SHA256).Hash) {
        throw "Installed executable does not match the release executable"
    }
    $testOpenCommand = Join-Path $testClassesRoot "MDPreviewer.md\shell\open\command"
    if (-not (Test-Path -LiteralPath $testOpenCommand)) {
        throw "Installer test did not register the Markdown open command"
    }
    if ((Get-Item -LiteralPath $testOpenCommand).GetValue("") -notlike "*$installedExe*") {
        throw "Installer test registered an unexpected Markdown open command"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $testStartMenu "MD Previewer.lnk"))) {
        throw "Installer test did not create the Start Menu shortcut"
    }
    if (-not (Test-Path -LiteralPath $testUninstallRoot)) {
        throw "Installer test did not create uninstall metadata"
    }

    $uninstaller = Join-Path $testInstall "uninstall-windows.ps1"
    $uninstallArguments = "-NoProfile -ExecutionPolicy Bypass -File $(Quote-ProcessArgument $uninstaller)"
    $uninstallProcess = Start-WithEnvironment "powershell.exe" $uninstallArguments $testEnvironment
    Wait-ForExit $uninstallProcess "uninstaller test"
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while (Test-Path -LiteralPath $testInstall) {
        if ([DateTime]::UtcNow -gt $deadline) {
            throw "Uninstaller test did not remove $testInstall"
        }
        Start-Sleep -Milliseconds 200
    }
    if ((Test-Path -LiteralPath $testOpenCommand) -or
        (Test-Path -LiteralPath $testStartMenu) -or
        (Test-Path -LiteralPath $testUninstallRoot)) {
        throw "Uninstaller test left shell integration behind"
    }
    Remove-Item -LiteralPath $testRegistryRoot -Recurse -Force -ErrorAction SilentlyContinue
    $startMenuDeadline = [DateTime]::UtcNow.AddSeconds(10)
    while (Test-Path -LiteralPath $testStartMenu) {
        try {
            Remove-Item -LiteralPath $testStartMenu -Recurse -Force -ErrorAction Stop
            break
        }
        catch {
            if ([DateTime]::UtcNow -gt $startMenuDeadline) {
                throw "Uninstaller test did not remove $testStartMenu"
            }
            Start-Sleep -Milliseconds 200
        }
    }
}

Remove-Item -LiteralPath $payload -Recurse -Force
Remove-Item -LiteralPath $sed -Force
$hashes = @($portableExe, $installer) | ForEach-Object {
    $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256
    "$($hash.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_))"
}
[IO.File]::WriteAllLines((Join-Path $dist "SHA256SUMS.txt"), $hashes, [Text.Encoding]::ASCII)

$exeSize = (Get-Item -LiteralPath $portableExe).Length
$installerSize = (Get-Item -LiteralPath $installer).Length
Write-Host "[build] Version: $version"
Write-Host "[build] Portable EXE: $portableExe ($exeSize bytes)"
Write-Host "[build] Installer: $installer ($installerSize bytes)"
Write-Host "[build] Checksums: $(Join-Path $dist 'SHA256SUMS.txt')"
