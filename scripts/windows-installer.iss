; MD Previewer 的 Windows 安装向导（Inno Setup 6）
;
; 由 scripts/build-windows.ps1 用 ISCC 编译。版本号、载荷目录、输出位置和注册表根
; 全部由构建脚本传入，安装器本身不重复定义这些事实。
;
; 关键产品决策：
; - 仍然只装当前用户（PrivilegesRequired=lowest），不弹 UAC，路径可由用户改
; - “打开方式”按格式勾选，只写 OpenWithProgids，不去抢系统默认程序
;   （Windows 10/11 会拦程序改 UserChoice，强设默认只会失败或回滚）

; 版本、载荷路径和注册表根由 scripts/build-windows.ps1 通过环境变量传入。
; 不用 ISCC 的 /Dname=value 是因为带空格的路径会被拆开。
#define AppVersion GetEnv("MD_PREVIEWER_ISCC_VERSION")
#define SourceDir GetEnv("MD_PREVIEWER_ISCC_SOURCE")
#define SourceRoot GetEnv("MD_PREVIEWER_ISCC_ROOT")
#define OutputDir GetEnv("MD_PREVIEWER_ISCC_OUTPUT")
#define OutputBaseFilename GetEnv("MD_PREVIEWER_ISCC_BASENAME")
#define ClassesSubkey GetEnv("MD_PREVIEWER_ISCC_CLASSES")
#define FileExtsSubkey GetEnv("MD_PREVIEWER_ISCC_FILEEXTS")
#define LegacyUninstallSubkey GetEnv("MD_PREVIEWER_ISCC_LEGACY_UNINSTALL")
#define GroupName GetEnv("MD_PREVIEWER_ISCC_GROUP")
#define ChineseMessagesFile GetEnv("MD_PREVIEWER_ISCC_CHINESE_MESSAGES")
; 右键菜单的 CLSID，必须和 association-prune.ps1、src/platform.rs 里的常量一致
#define ShellClsid "7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902"
; 带花括号的写法给注册表用：Inno 里 "{{" 展开成一个 "{"
#define ShellClsidKey "{{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}"

[Setup]
AppId={{8C3A2E51-6D47-4D2C-9C0A-2B4F7D8E1A63}
AppName=MD Previewer
AppVersion={#AppVersion}
AppVerName=MD Previewer {#AppVersion}
AppPublisher=ArnoldRedman
AppPublisherURL=https://github.com/ArnoldRedman/MD-Previewer
AppSupportURL=https://github.com/ArnoldRedman/MD-Previewer/issues
DefaultDirName={localappdata}\Programs\MD Previewer
DefaultGroupName={#GroupName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
SetupIconFile={#SourceRoot}\assets\icon.ico
UninstallDisplayIcon={app}\md-previewer.exe
UninstallDisplayName=MD Previewer
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; 更新时旧版本可能还在跑：让 Restart Manager 负责关闭，装完不自动重启
CloseApplications=yes
RestartApplications=no

[Languages]
; 中文翻译文件随仓库放在 scripts/languages；官方安装包不自带，所以不能写 compiler:
Name: "chs"; MessagesFile: "{#ChineseMessagesFile}"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
; 不带语言前缀的是英文兜底，chs.* 是中文向导用的那套
GroupExtras=Additional tasks
GroupAssoc=Add to the “Open with” list (registers only, never changes the system default)
TaskDesktopIcon=Create a desktop shortcut
TaskStartMenu=Create a Start Menu shortcut
TaskAssocMarkdown=Markdown: .md .markdown .mdown .mkd
TaskAssocText=Plain text: .txt .log
TaskAssocConfig=Config and data: .json .toml .yaml .yml
TaskAssocEnv=.env
ShellVerb=Edit with MD Previewer
chs.GroupExtras=附加任务
chs.GroupAssoc=加入“打开方式”（只登记，不改变系统默认程序）
chs.TaskDesktopIcon=创建桌面快捷方式
chs.TaskStartMenu=创建开始菜单快捷方式
chs.TaskAssocMarkdown=Markdown：.md .markdown .mdown .mkd
chs.TaskAssocText=纯文本：.txt .log
chs.TaskAssocConfig=配置与数据：.json .toml .yaml .yml
chs.TaskAssocEnv=.env
chs.ShellVerb=以 MD Previewer 编辑

[Tasks]
Name: "desktopicon"; Description: "{cm:TaskDesktopIcon}"; GroupDescription: "{cm:GroupExtras}"; Flags: unchecked
Name: "startmenu"; Description: "{cm:TaskStartMenu}"; GroupDescription: "{cm:GroupExtras}"
Name: "assoc_markdown"; Description: "{cm:TaskAssocMarkdown}"; GroupDescription: "{cm:GroupAssoc}"
Name: "assoc_text"; Description: "{cm:TaskAssocText}"; GroupDescription: "{cm:GroupAssoc}"
Name: "assoc_config"; Description: "{cm:TaskAssocConfig}"; GroupDescription: "{cm:GroupAssoc}"
Name: "assoc_env"; Description: "{cm:TaskAssocEnv}"; GroupDescription: "{cm:GroupAssoc}"

[Files]
Source: "{#SourceDir}\md-previewer.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\md-previewer-shell.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\association-prune.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\NOTICE"; DestDir: "{app}"; Flags: ignoreversion

[InstallDelete]
; 1.6.1 及更早的 IExpress 安装器留下的脚本，换成本安装器后不再需要
Type: files; Name: "{app}\install-windows.ps1"
Type: files; Name: "{app}\install-windows.cmd"
Type: files; Name: "{app}\uninstall-windows.ps1"
Type: files; Name: "{app}\uninstall.cmd"

[Icons]
Name: "{autoprograms}\{#GroupName}\MD Previewer"; Filename: "{app}\md-previewer.exe"; WorkingDir: "{app}"; Comment: "MD Previewer Markdown reader"; Tasks: startmenu
Name: "{autoprograms}\{#GroupName}\Uninstall MD Previewer"; Filename: "{uninstallexe}"; Tasks: startmenu
Name: "{autodesktop}\MD Previewer"; Filename: "{app}\md-previewer.exe"; WorkingDir: "{app}"; Comment: "MD Previewer Markdown reader"; Tasks: desktopicon

[Registry]
; 文档 ProgID
Root: HKCU; Subkey: "{#ClassesSubkey}\MDPreviewer.md"; ValueType: string; ValueName: ""; ValueData: "MD Previewer Markdown Document"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#ClassesSubkey}\MDPreviewer.md\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\md-previewer.exe"",0"
Root: HKCU; Subkey: "{#ClassesSubkey}\MDPreviewer.md\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\md-previewer.exe"" ""%1"""

; “打开方式”候选：按勾选的格式写 Classes 和 FileExts 两处，Explorer 两个都读
Root: HKCU; Subkey: "{#ClassesSubkey}\.md\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#FileExtsSubkey}\.md\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\.markdown\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#FileExtsSubkey}\.markdown\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\.mdown\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#FileExtsSubkey}\.mdown\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\.mkd\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#FileExtsSubkey}\.mkd\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\.txt\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_text
Root: HKCU; Subkey: "{#FileExtsSubkey}\.txt\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_text
Root: HKCU; Subkey: "{#ClassesSubkey}\.log\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_text
Root: HKCU; Subkey: "{#FileExtsSubkey}\.log\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_text
Root: HKCU; Subkey: "{#ClassesSubkey}\.json\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#FileExtsSubkey}\.json\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\.toml\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#FileExtsSubkey}\.toml\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\.yaml\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#FileExtsSubkey}\.yaml\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\.yml\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#FileExtsSubkey}\.yml\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\.env\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_env
Root: HKCU; Subkey: "{#FileExtsSubkey}\.env\OpenWithProgids"; ValueType: string; ValueName: "MDPreviewer.md"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_env

; “打开方式”对话框里的应用条目，SupportedTypes 决定它出现在哪些扩展名的候选里
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "MD Previewer"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\md-previewer.exe"" ""%1"""
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".md"; ValueData: ""; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".markdown"; ValueData: ""; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".mdown"; ValueData: ""; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".mkd"; ValueData: ""; Tasks: assoc_markdown
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".txt"; ValueData: ""; Tasks: assoc_text
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".log"; ValueData: ""; Tasks: assoc_text
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".json"; ValueData: ""; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".toml"; ValueData: ""; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".yaml"; ValueData: ""; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".yml"; ValueData: ""; Tasks: assoc_config
Root: HKCU; Subkey: "{#ClassesSubkey}\Applications\md-previewer.exe\SupportedTypes"; ValueType: string; ValueName: ".env"; ValueData: ""; Tasks: assoc_env

; 右键「以 MD Previewer 编辑」：外壳菜单项 + 指向本进程内处理器的 CLSID
Root: HKCU; Subkey: "{#ClassesSubkey}\*\shell\MDPreviewer"; ValueType: string; ValueName: ""; ValueData: "{cm:ShellVerb}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#ClassesSubkey}\*\shell\MDPreviewer"; ValueType: string; ValueName: "NeverDefault"; ValueData: ""
Root: HKCU; Subkey: "{#ClassesSubkey}\*\shell\MDPreviewer"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\md-previewer.exe"",0"
Root: HKCU; Subkey: "{#ClassesSubkey}\*\shell\MDPreviewer"; ValueType: string; ValueName: "ExplorerCommandHandler"; ValueData: "{#ShellClsidKey}"
Root: HKCU; Subkey: "{#ClassesSubkey}\CLSID\{#ShellClsidKey}"; ValueType: string; ValueName: ""; ValueData: "MD Previewer"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#ClassesSubkey}\CLSID\{#ShellClsidKey}"; ValueType: string; ValueName: "Exe"; ValueData: "{app}\md-previewer.exe"
Root: HKCU; Subkey: "{#ClassesSubkey}\CLSID\{#ShellClsidKey}"; ValueType: string; ValueName: "Title"; ValueData: "{cm:ShellVerb}"
Root: HKCU; Subkey: "{#ClassesSubkey}\CLSID\{#ShellClsidKey}\InProcServer32"; ValueType: string; ValueName: ""; ValueData: "{app}\md-previewer-shell.dll"
Root: HKCU; Subkey: "{#ClassesSubkey}\CLSID\{#ShellClsidKey}\InProcServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Apartment"

[Code]
// PS 单引号字面量：路径和扩展名进 PowerShell 时统一用这层包一下
function Quote(const Value: String): String;
begin
  Result := '''' + Value + '''';
end;

// association-prune.ps1 要扫的扩展名；和 [Registry] 里按格式勾选的条目保持同一份清单
function ExtensionListPs(): String;
begin
  Result := '@(' + Quote('.md') + ',' + Quote('.markdown') + ',' + Quote('.mdown') + ',' +
    Quote('.mkd') + ',' + Quote('.txt') + ',' + Quote('.json') + ',' + Quote('.toml') + ',' +
    Quote('.yaml') + ',' + Quote('.yml') + ',' + Quote('.log') + ',' + Quote('.env') + ')';
end;

// 安装和卸载都要跑一次 prune：Inno 只会清掉自己写过的键值，旧版本按 exe 文件名注册的
// Applications 条目和 FileExts\*\OpenWithList 里的陈旧名字得靠它扫
function RunAssociationPrune(IncludeCanonical: Boolean): Boolean;
var
  Params, PruneScript: String;
  ResultCode: Integer;
begin
  Result := True;
  PruneScript := ExpandConstant('{app}\association-prune.ps1');
  if not FileExists(PruneScript) then begin
    Log('association-prune.ps1 missing, skipping prune');
    exit;
  end;
  Params := '-NoProfile -ExecutionPolicy Bypass -Command ". ' + Quote(PruneScript) + '; ' +
    'Remove-LegacyPreviewerEntries -ClassesRoot ' + Quote('HKCU:\{#ClassesSubkey}') +
    ' -FileExtsRoot ' + Quote('HKCU:\{#FileExtsSubkey}') +
    ' -Extensions ' + ExtensionListPs();
  if IncludeCanonical then
    Params := Params + ' -IncludeCanonical';
  Params := Params + '"';
  if (not Exec('powershell.exe', Params, '', SW_HIDE, ewWaitUntilTerminated, ResultCode)) or (ResultCode <> 0) then begin
    Log('association prune failed with exit code ' + IntToStr(ResultCode));
    Result := False;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then begin
    // 1.6.1 的 IExpress 版本自己写了一条卸载项，先删掉，否则控制面板里会同时出现两条
    if '{#LegacyUninstallSubkey}' <> '' then
      RegDeleteKeyIncludingSubkeys(HKCU, '{#LegacyUninstallSubkey}');
  end else if CurStep = ssPostInstall then
    // 装完把旧版本留下的重复“打开方式”行清掉：安装阶段只从这里 prune 一次
    RunAssociationPrune(False);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RunAssociationPrune(True);
end;
