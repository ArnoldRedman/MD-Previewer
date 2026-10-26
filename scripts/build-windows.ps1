param(
    [switch]$SkipInstallerTest,
    # 目标架构：默认取操作系统架构（ARM64 Windows 上即便 PowerShell 跑在 x64 模拟里也报 Arm64）
    [ValidateSet("x64", "arm64")]
    [string]$Arch
)

$ErrorActionPreference = "Stop"
Import-Module Microsoft.PowerShell.Utility -ErrorAction SilentlyContinue

$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location -LiteralPath $root
$cargo = (Get-Command cargo.exe -ErrorAction Stop).Source

if ([string]::IsNullOrWhiteSpace($Arch)) {
    $Arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq
        [System.Runtime.InteropServices.Architecture]::Arm64) { "arm64" } else { "x64" }
}

$dist = Join-Path $root "dist"
$payload = Join-Path $dist "payload"
$releaseExe = Join-Path $root "target\release\md-previewer.exe"
# 资产名带架构：x64 的三个名字保持历史拼写，落地页和热更后缀都按它匹配
$portableExe = Join-Path $dist "MD-Previewer-windows-$Arch.exe"
$installerName = if ($Arch -eq "arm64") { "MD-Previewer-Setup-arm64" } else { "MD-Previewer-Setup" }
$installer = Join-Path $dist "$installerName.exe"
$testInstall = Join-Path $dist "installer-test"
$testRegistryRoot = "HKCU:\Software\MDPreviewerBuildTest"
$testClassesRoot = Join-Path $testRegistryRoot "Classes"
$testFileExtsRoot = Join-Path $testRegistryRoot "FileExts"
$testUninstallRoot = Join-Path $testRegistryRoot "Uninstall"
$testGroupName = "MDPreviewerBuildTest"
$testStartMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$testGroupName"
$testDesktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) "MD Previewer.lnk"

# 已在运行的 md-previewer 会锁住 target\release 里的 exe，让 cargo 报一句难懂的
# “failed to remove file ... 拒绝访问”；先查一次，直接告诉要关掉哪个进程
$runningApp = @(Get-Process md-previewer -ErrorAction SilentlyContinue)
if ($runningApp.Count -gt 0) {
    $ids = ($runningApp | ForEach-Object { $_.Id }) -join ", "
    throw "Close MD Previewer before building; still running: PID $ids"
}

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

Write-Host "[build] Running Rust tests"
Invoke-Cargo @("test", "--locked")

Write-Host "[build] Building shell extension"
Invoke-Cargo @("build", "--release", "--locked", "-p", "md-previewer-shell")
$shellDll = Join-Path $root "target\release\md_previewer_shell.dll"
if (-not (Test-Path -LiteralPath $shellDll)) {
    throw "Shell extension was not created: $shellDll"
}
$env:MD_PREVIEWER_SHELL_DLL = $shellDll
Write-Host "[build] Building release executable"
Invoke-Cargo @("build", "--release", "--locked", "-p", "md-previewer")
Remove-Item Env:MD_PREVIEWER_SHELL_DLL
if (-not (Test-Path -LiteralPath $releaseExe)) {
    throw "Release executable was not created: $releaseExe"
}

# cargo 可能被 x64 模拟 shell 拉起来而产出 x64 二进制；真查一次 PE 机器类型，
# 别把 x64 包标成 arm64 传上去
function Assert-PeMachine([string]$Path, [string]$Expected) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $peOffset = [System.BitConverter]::ToInt32($bytes, 0x3C)
    $machine = [System.BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $want = if ($Expected -eq "arm64") { 0xAA64 } else { 0x8664 }
    if ($machine -ne $want) {
        throw "Expected $Expected PE machine 0x$('{0:X}' -f $want), got 0x$('{0:X}' -f $machine): cargo built for the wrong host arch"
    }
}

Assert-PeMachine $releaseExe $Arch
# Read the version from the resource build.rs embeds, so Cargo.toml is parsed in exactly one place (verify.sh)
$version = (Get-Item -LiteralPath $releaseExe).VersionInfo.ProductVersion
if ([string]::IsNullOrWhiteSpace($version)) {
    throw "Release executable is missing embedded ProductVersion"
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
Copy-Item -LiteralPath $shellDll -Destination (Join-Path $payload "md-previewer-shell.dll")
foreach ($file in @("LICENSE", "NOTICE", "association-prune.ps1")) {
    $source = if ($file -in @("LICENSE", "NOTICE")) {
        Join-Path $root $file
    }
    else {
        Join-Path $root "scripts\$file"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $payload $file)
}

# 安装向导由 Inno Setup 6 编译（scripts\windows-installer.iss）。安装位置、快捷方式和
# 按格式勾选的文件关联都长在 .iss 里，这里只负责把版本、载荷和输出路径传进去。
function Resolve-Iscc {
    $candidates = @()
    if (-not [string]::IsNullOrWhiteSpace($env:INNO_SETUP_DIR)) {
        $candidates += (Join-Path $env:INNO_SETUP_DIR "ISCC.exe")
    }
    foreach ($base in @(${env:ProgramFiles(x86)}, $env:ProgramFiles, $env:LOCALAPPDATA)) {
        if ([string]::IsNullOrWhiteSpace($base)) {
            continue
        }
        $candidates += (Join-Path $base "Inno Setup 6\ISCC.exe")
        $candidates += (Join-Path $base "Programs\Inno Setup 6\ISCC.exe")
    }
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    throw "Inno Setup 6 was not found. Install it with: winget install -e --id JRSoftware.InnoSetup"
}

$iscc = Resolve-Iscc
# 中文向导的翻译文件跟仓库走：Inno 官方包只带英文
$chineseMessages = Join-Path $root "scripts\languages\ChineseSimplified.isl"
if (-not (Test-Path -LiteralPath $chineseMessages)) {
    throw "Installer translation is missing: $chineseMessages"
}

# 版本、载荷、输出位置和注册表根都走环境变量，避免 /D 参数在带空格的路径上被拆开
function Compile-Installer([string]$OutputDir, [string]$OutputBaseName, [hashtable]$Overrides) {
    $env:MD_PREVIEWER_ISCC_VERSION = $version
    $env:MD_PREVIEWER_ISCC_SOURCE = $payload
    $env:MD_PREVIEWER_ISCC_ROOT = $root
    $env:MD_PREVIEWER_ISCC_OUTPUT = $OutputDir
    $env:MD_PREVIEWER_ISCC_BASENAME = $OutputBaseName
    $env:MD_PREVIEWER_ISCC_CLASSES = "Software\Classes"
    $env:MD_PREVIEWER_ISCC_FILEEXTS = "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts"
    # 1.6.1 的 IExpress 安装器写的那条卸载项，安装时删掉，免得控制面板出现两条
    $env:MD_PREVIEWER_ISCC_LEGACY_UNINSTALL = "Software\Microsoft\Windows\CurrentVersion\Uninstall\MDPreviewer"
    $env:MD_PREVIEWER_ISCC_GROUP = "MD Previewer"
    # x64 用 x64compatible（ARM64 Windows 上靠模拟也能装），arm64 只允许 ARM64
    $env:MD_PREVIEWER_ISCC_ARCH = if ($Arch -eq "arm64") { "arm64" } else { "x64compatible" }
    $env:MD_PREVIEWER_ISCC_CHINESE_MESSAGES = $chineseMessages
    foreach ($name in $Overrides.Keys) {
        Set-Item -Path "env:$name" -Value ([string]$Overrides[$name])
    }
    & $iscc "/Q" (Join-Path $root "scripts\windows-installer.iss")
    if ($LASTEXITCODE -ne 0) {
        throw "ISCC failed with exit code $LASTEXITCODE"
    }
}

Write-Host "[build] Creating installer with Inno Setup"
Compile-Installer $dist $installerName @{}
if (-not (Test-Path -LiteralPath $installer)) {
    throw "Installer was not created: $installer"
}

if (-not $SkipInstallerTest) {
    Write-Host "[build] Testing silent install and uninstall"
    $testOutput = Join-Path $dist "installer-test-dist"
    Remove-Item -LiteralPath $testOutput -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $testOutput -Force | Out-Null
    $testInstaller = Join-Path $testOutput "MD-Previewer-Setup-test.exe"
    # 测试专用编译：注册表根指向 HKCU\Software\MDPreviewerBuildTest、开始菜单换成独立分组名，
    # 自测就不会碰到真实安装留下的关联、快捷方式和卸载项
    Compile-Installer $testOutput "MD-Previewer-Setup-test" @{
        MD_PREVIEWER_ISCC_CLASSES = "Software\MDPreviewerBuildTest\Classes"
        MD_PREVIEWER_ISCC_FILEEXTS = "Software\MDPreviewerBuildTest\FileExts"
        MD_PREVIEWER_ISCC_LEGACY_UNINSTALL = "Software\MDPreviewerBuildTest\Uninstall"
        MD_PREVIEWER_ISCC_GROUP = "MDPreviewerBuildTest"
    }
    # 开发机上可能已经装着一份 MD Previewer：先记下现状，断言只看"这次安装新增了什么"
    $uninstallRootKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall"
    $uninstallKeysBefore = @(Get-ChildItem -LiteralPath $uninstallRootKey -ErrorAction SilentlyContinue | ForEach-Object { $_.PSChildName })
    $desktopShortcutFree = -not (Test-Path -LiteralPath $testDesktopShortcut)
    $testTasks = "startmenu,assoc_markdown,assoc_text,assoc_config,assoc_env"
    if ($desktopShortcutFree) {
        $testTasks = "desktopicon," + $testTasks
    }
    try {
        # 先埋旧版本的残留：另一个 exe 名下的 Applications 条目、旧 ProgID、OpenWithList
        # 里的陈旧 exe 名，以及 1.6.1 那套 IExpress 安装器写下的卸载项
        $legacyApp = Join-Path $testClassesRoot "Applications\MD-Previewer-windows-x64.exe"
        New-Item -Path (Join-Path $legacyApp "shell\open\command") -Force | Out-Null
        New-ItemProperty -Path $legacyApp -Name 'FriendlyAppName' -Value 'MD Previewer' -PropertyType String -Force | Out-Null
        $legacyProgId = Join-Path $testClassesRoot "MDPreviewer.txt"
        New-Item -Path $legacyProgId -Force | Out-Null
        $legacyProgIdRef = Join-Path $testClassesRoot ".txt\OpenWithProgids"
        New-Item -Path $legacyProgIdRef -Force | Out-Null
        New-ItemProperty -Path $legacyProgIdRef -Name 'MDPreviewer.txt' -Value '' -PropertyType String -Force | Out-Null
        $legacyList = Join-Path $testFileExtsRoot ".md\OpenWithList"
        New-Item -Path $legacyList -Force | Out-Null
        New-ItemProperty -Path $legacyList -Name 'a' -Value 'rider64.exe' -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $legacyList -Name 'b' -Value 'md-previewer.exe' -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $legacyList -Name 'c' -Value 'MD-Previewer-windows-x64.exe' -PropertyType String -Force | Out-Null
        New-ItemProperty -Path $legacyList -Name 'MRUList' -Value 'bca' -PropertyType String -Force | Out-Null

        $installArguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR="{0}" /GROUP="MDPreviewerBuildTest" /MERGETASKS="{1}"' -f $testInstall, $testTasks
        $installProcess = Start-Process -FilePath $testInstaller -ArgumentList $installArguments -Wait -PassThru
        if ($installProcess.ExitCode -ne 0) {
            throw "installer test failed with exit code $($installProcess.ExitCode)"
        }
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
        $testShell = Join-Path $testClassesRoot "*\shell\MDPreviewer"
        $testHandler = (Get-Item -LiteralPath $testShell).GetValue("ExplorerCommandHandler")
        if ($testHandler -ne "{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}") {
            throw "Installer test did not register the Edit with MD Previewer command"
        }
        $installedDll = Join-Path $testInstall "md-previewer-shell.dll"
        $testInProc = Join-Path $testClassesRoot "CLSID\{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}\InProcServer32"
        if ((Get-Item -LiteralPath $testInProc).GetValue("") -ne $installedDll) {
            throw "Installer test registered an unexpected shell extension path"
        }
        # 勾选的两个格式组（Markdown / 纯文本）都必须真的写进候选列表
        $testEnvOpen = Join-Path $testClassesRoot ".env\OpenWithProgids"
        if ((Get-Item -LiteralPath $testEnvOpen).GetValueNames() -notcontains "MDPreviewer.md") {
            throw "Installer test did not advertise MD Previewer for .env"
        }
        if ((Get-Item -LiteralPath $legacyProgIdRef).GetValueNames() -notcontains "MDPreviewer.md") {
            throw "Installer test did not advertise MD Previewer for .txt"
        }
        # Only one MD Previewer entry may survive: the legacy ones must be gone
        $previewerApps = @(Get-ChildItem -LiteralPath (Join-Path $testClassesRoot "Applications") | Where-Object {
                (Get-ItemProperty -LiteralPath $_.PSPath -Name 'FriendlyAppName' -ErrorAction SilentlyContinue).FriendlyAppName -eq 'MD Previewer'
            })
        if ($previewerApps.Count -ne 1 -or $previewerApps[0].PSChildName -ne 'md-previewer.exe') {
            throw "Expected exactly one MD Previewer app entry, got: $(($previewerApps | ForEach-Object { $_.PSChildName }) -join ', ')"
        }
        if (Test-Path -LiteralPath $legacyProgId) {
            throw "Installer test did not remove the legacy ProgID"
        }
        if ((Get-Item -LiteralPath $legacyProgIdRef).GetValueNames() -contains 'MDPreviewer.txt') {
            throw "Installer test left the legacy ProgID in .txt OpenWithProgids"
        }
        $list = Get-Item -LiteralPath $legacyList
        if ($list.GetValue('MRUList') -ne 'ab' -or $list.GetValue('a') -ne 'md-previewer.exe' -or
            $list.GetValue('b') -ne 'rider64.exe') {
            throw "Installer test pruned the OpenWithList wrong: MRUList=$($list.GetValue('MRUList')) a=$($list.GetValue('a')) b=$($list.GetValue('b'))"
        }
        if (-not (Test-Path -LiteralPath (Join-Path $testStartMenu "MD Previewer.lnk"))) {
            throw "Installer test did not create the Start Menu shortcut"
        }
        if ($desktopShortcutFree -and -not (Test-Path -LiteralPath $testDesktopShortcut)) {
            throw "Installer test did not create the desktop shortcut"
        }
        if (Test-Path -LiteralPath $testUninstallRoot) {
            throw "Installer test did not remove the IExpress uninstall entry"
        }
        $newUninstallKeys = @(Get-ChildItem -LiteralPath $uninstallRootKey -ErrorAction SilentlyContinue |
            ForEach-Object { $_.PSChildName } | Where-Object { $uninstallKeysBefore -notcontains $_ })
        if ($newUninstallKeys.Count -ne 1) {
            throw "Installer test created $($newUninstallKeys.Count) uninstall entries instead of one: $($newUninstallKeys -join ', ')"
        }

        $uninstaller = Join-Path $testInstall "unins000.exe"
        if (-not (Test-Path -LiteralPath $uninstaller)) {
            throw "Installer test did not create the uninstaller"
        }
        $uninstallProcess = Start-Process -FilePath $uninstaller -ArgumentList "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART" -Wait -PassThru
        if ($uninstallProcess.ExitCode -ne 0) {
            throw "uninstaller test failed with exit code $($uninstallProcess.ExitCode)"
        }
        # 卸载器会把自己复制到临时目录再跑，等它真正把安装目录删掉
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        while (Test-Path -LiteralPath $testInstall) {
            if ([DateTime]::UtcNow -gt $deadline) {
                throw "Uninstaller test did not remove $testInstall"
            }
            Start-Sleep -Milliseconds 200
        }
        if ((Test-Path -LiteralPath $testOpenCommand) -or
            (Test-Path -LiteralPath $testShell) -or
            (Test-Path -LiteralPath $testInProc) -or
            (Test-Path -LiteralPath $testStartMenu) -or
            ($desktopShortcutFree -and (Test-Path -LiteralPath $testDesktopShortcut))) {
            throw "Uninstaller test left shell integration behind"
        }
        if ((Get-Item -LiteralPath $legacyProgIdRef).GetValueNames() -contains 'MDPreviewer.md') {
            throw "Uninstaller test left MD Previewer in .txt OpenWithProgids"
        }
        $keysLeft = @(Get-ChildItem -LiteralPath $uninstallRootKey -ErrorAction SilentlyContinue |
            ForEach-Object { $_.PSChildName } | Where-Object { $newUninstallKeys -contains $_ })
        if ($keysLeft.Count -gt 0) {
            throw "Uninstaller test left the uninstall metadata behind: $($keysLeft -join ', ')"
        }
        # Uninstall must not leave our names behind in the candidate list or in
        # Applications, or the Open With dialog still lists a row. The list itself
        # may legitimately survive for other apps.
        $leftoverNames = @()
        if (Test-Path -LiteralPath $legacyList) {
            $listKey = Get-Item -LiteralPath $legacyList
            $leftoverNames = @($listKey.GetValueNames() | Where-Object { $_.Length -eq 1 } |
                ForEach-Object { [string]$listKey.GetValue($_) })
        }
        if ($leftoverNames -contains 'md-previewer.exe') {
            throw "Uninstaller test left MD Previewer in the OpenWithList: $($leftoverNames -join ', ')"
        }
        $appsLeft = @()
        $appsRoot = Join-Path $testClassesRoot "Applications"
        if (Test-Path -LiteralPath $appsRoot) {
            $appsLeft = @(Get-ChildItem -LiteralPath $appsRoot | Where-Object {
                    (Get-ItemProperty -LiteralPath $_.PSPath -Name 'FriendlyAppName' -ErrorAction SilentlyContinue).FriendlyAppName -eq 'MD Previewer'
                } | ForEach-Object { $_.PSChildName })
        }
        if ($appsLeft.Count -gt 0) {
            throw "Uninstaller test left MD Previewer app entries: $($appsLeft -join ', ')"
        }
    }
    finally {
        # Never leave test registry keys or temp dirs behind on a failed assertion; also sweeps older failures
        Remove-Item -LiteralPath $testRegistryRoot -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $testStartMenu -Recurse -Force -ErrorAction SilentlyContinue
        if ($desktopShortcutFree) {
            Remove-Item -LiteralPath $testDesktopShortcut -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $testInstall -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $testOutput -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Remove-Item -LiteralPath $payload -Recurse -Force
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
