#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef RepoRoot
  #define RepoRoot ".."
#endif
#ifndef ReleaseDir
  #define ReleaseDir RepoRoot + "\target\release"
#endif
#ifndef OutputDir
  #define OutputDir RepoRoot + "\dist"
#endif

#define AppName "FastZIP"
#define AppPublisher "FastZIP"
#define MainExeName "fastzip.exe"
#define CliExeName "fastzip-cli.exe"
#define IconFileName "fastzip.ico"

[Setup]
AppId={{9EEC114A-C5BA-47A3-84FD-E5732D22506C}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\FastZIP
DefaultGroupName=FastZIP
UninstallDisplayIcon={app}\{#IconFileName}
ChangesAssociations=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LanguageDetectionMethod=uilanguage
ShowLanguageDialog=auto
UsePreviousLanguage=no
DisableProgramGroupPage=yes
CloseApplications=yes
RestartApplications=no
OutputDir={#OutputDir}
OutputBaseFilename=FastZIP-Setup-{#AppVersion}
SetupLogging=yes
SetupIconFile={#RepoRoot}\assets\{#IconFileName}
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription=FastZIP Installer

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "ChineseSimplified.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "arabic"; MessagesFile: "compiler:Languages\Arabic.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:TaskDesktopIcon}"; Flags: unchecked
Name: "fileassoc"; Description: "{cm:TaskFileAssociations}"; Flags: checkedonce
Name: "contextmenu"; Description: "{cm:TaskContextMenu}"; Flags: checkedonce
Name: "autostart"; Description: "{cm:TaskAutostart}"; Flags: unchecked

[Files]
Source: "{#ReleaseDir}\{#MainExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ReleaseDir}\{#CliExeName}"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#RepoRoot}\assets\{#IconFileName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\FastZIP"; Filename: "{app}\{#MainExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\{#IconFileName}"
Name: "{group}\Uninstall FastZIP"; Filename: "{uninstallexe}"
Name: "{autodesktop}\FastZIP"; Filename: "{app}\{#MainExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\{#IconFileName}"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\FastZIP.Archive"; ValueType: string; ValueData: "{cm:ArchiveTypeName}"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\FastZIP.Archive"; ValueName: "FriendlyTypeName"; ValueType: string; ValueData: "{cm:ArchiveTypeName}"; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\FastZIP.Archive"; ValueName: "AppUserModelID"; ValueType: string; ValueData: "FastZIP"; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\FastZIP.Archive\DefaultIcon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Flags: uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\FastZIP.Archive\shell\open\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" --archive ""%1"""; Flags: uninsdeletekeyifempty; Tasks: fileassoc

Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe"; ValueName: "FriendlyAppName"; ValueType: string; ValueData: "FastZIP"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\shell\open\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" --archive ""%1"""; Flags: uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".7z"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".zip"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tar"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tar.gz"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tgz"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tar.bz2"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tbz2"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tar.xz"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".txz"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".gz"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".bz2"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".bzip2"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".xz"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".zst"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".zstd"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tar.zst"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tzst"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".lz4"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tar.lz4"; ValueType: string; ValueData: ""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\fastzip.exe\SupportedTypes"; ValueName: ".tlz4"; ValueType: string; ValueData: ""; Tasks: fileassoc

Root: HKA; Subkey: "Software\Classes\.7z\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.7z\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.zip\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.zip\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.gz\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.gz\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tgz\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tgz\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.bz2\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.bz2\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tbz2\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tbz2\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.xz\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.xz\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.txz\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.txz\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.gz\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.gz\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.bz2\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.bz2\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.bzip2\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.bzip2\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.xz\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.xz\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.zst\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.zst\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.zstd\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.zstd\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.zst\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.zst\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tzst\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tzst\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.lz4\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.lz4\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.lz4\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tar.lz4\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tlz4\OpenWithProgids"; ValueName: "FastZIP.Archive"; ValueType: string; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.tlz4\OpenWithList\fastzip.exe"; ValueType: none; Flags: uninsdeletekey; Tasks: fileassoc

Root: HKA; Subkey: "Software\FastZIP\Capabilities"; ValueName: "ApplicationName"; ValueType: string; ValueData: "FastZIP"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities"; ValueName: "ApplicationDescription"; ValueType: string; ValueData: "{cm:ApplicationDescription}"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".7z"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".zip"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tar"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tar.gz"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tgz"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tar.bz2"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tbz2"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tar.xz"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".txz"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".gz"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".bz2"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".bzip2"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".xz"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".zst"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".zstd"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tar.zst"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tzst"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".lz4"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tar.lz4"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\FastZIP\Capabilities\FileAssociations"; ValueName: ".tlz4"; ValueType: string; ValueData: "FastZIP.Archive"; Tasks: fileassoc
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueName: "FastZIP"; ValueType: string; ValueData: "Software\FastZIP\Capabilities"; Flags: uninsdeletevalue; Tasks: fileassoc

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo"; ValueName: "MUIVerb"; ValueType: string; ValueData: "{cm:ContextMenuRootVerb}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo"; ValueName: "SubCommands"; ValueType: string; ValueData: ""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zip"; ValueName: "MUIVerb"; ValueType: string; ValueData: "ZIP"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zip"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zip"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zip\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format zip ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\sevenzip"; ValueName: "MUIVerb"; ValueType: string; ValueData: "7Z"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\sevenzip"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\sevenzip"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\sevenzip\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format 7z ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tar"; ValueName: "MUIVerb"; ValueType: string; ValueData: "TAR"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tar"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tar"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tar\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format tar ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\targz"; ValueName: "MUIVerb"; ValueType: string; ValueData: "TAR.GZ"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\targz"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\targz"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\targz\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format tar.gz ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarbz2"; ValueName: "MUIVerb"; ValueType: string; ValueData: "TAR.BZ2"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarbz2"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarbz2"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarbz2\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format tar.bz2 ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarxz"; ValueName: "MUIVerb"; ValueType: string; ValueData: "TAR.XZ"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarxz"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarxz"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarxz\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format tar.xz ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\gz"; ValueName: "MUIVerb"; ValueType: string; ValueData: "GZ"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\gz"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\gz"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\gz\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format gz ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\bz2"; ValueName: "MUIVerb"; ValueType: string; ValueData: "BZ2"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\bz2"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\bz2"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\bz2\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format bz2 ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\xz"; ValueName: "MUIVerb"; ValueType: string; ValueData: "XZ"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\xz"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\xz"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\xz\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format xz ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zst"; ValueName: "MUIVerb"; ValueType: string; ValueData: "ZST"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zst"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zst"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\zst\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format zst ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarzst"; ValueName: "MUIVerb"; ValueType: string; ValueData: "TAR.ZST"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarzst"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarzst"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarzst\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format tar.zst ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\lz4"; ValueName: "MUIVerb"; ValueType: string; ValueData: "LZ4"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\lz4"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\lz4"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\lz4\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format lz4 ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarlz4"; ValueName: "MUIVerb"; ValueType: string; ValueData: "TAR.LZ4"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarlz4"; ValueName: "Icon"; ValueType: string; ValueData: "{app}\{#IconFileName}"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarlz4"; ValueName: "MultiSelectModel"; ValueType: string; ValueData: "Player"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\AllFilesystemObjects\shell\FastZIPCompressTo\shell\tarlz4\command"; ValueType: string; ValueData: """{app}\{#MainExeName}"" shell-compress --format tar.lz4 ""%1"""; Flags: uninsdeletekeyifempty; Tasks: contextmenu

Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "FastZIP"; ValueType: string; ValueData: """{app}\{#MainExeName}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#MainExeName}"; Description: "{cm:LaunchFastZIP}"; Flags: nowait postinstall skipifsilent unchecked

[CustomMessages]
TaskDesktopIcon=Create a desktop shortcut
TaskFileAssociations=Register archive associations and Open With entries
TaskDefaultApps=Open Windows Default Apps after setup
TaskContextMenu=Add the Explorer compression context menu
TaskAutostart=Start FastZIP when I sign in
LaunchFastZIP=Launch FastZIP
ArchiveTypeName=FastZIP Archive
ApplicationDescription=FastZIP archive manager
ContextMenuRootVerb=Compress with FastZIP to
AppLanguagePageTitle=Application Language
AppLanguagePageDescription=Choose the default language used by FastZIP after installation.
AppLanguagePrompt=Default language for the FastZIP app and message windows:
LanguageOptionEnglish=English
LanguageOptionChineseSimplified=简体中文
LanguageOptionJapanese=日本語
LanguageOptionKorean=한국어
LanguageOptionFrench=Français
LanguageOptionGerman=Deutsch
LanguageOptionSpanish=Español
LanguageOptionItalian=Italiano
LanguageOptionPortugueseBrazil=Português (Brasil)
LanguageOptionRussian=Русский
LanguageOptionArabic=العربية
LanguageOptionTurkish=Türkçe
chinesesimplified.TaskDesktopIcon=创建桌面快捷方式
chinesesimplified.TaskFileAssociations=注册压缩包关联和“打开方式”
chinesesimplified.TaskDefaultApps=安装后打开 Windows 默认应用设置
chinesesimplified.TaskContextMenu=添加资源管理器右键压缩菜单
chinesesimplified.TaskAutostart=登录 Windows 时自动启动 FastZIP
chinesesimplified.LaunchFastZIP=启动 FastZIP
chinesesimplified.ArchiveTypeName=FastZIP 压缩包
chinesesimplified.ApplicationDescription=FastZIP 压缩文件管理器
chinesesimplified.ContextMenuRootVerb=使用 FastZIP 压缩到
chinesesimplified.AppLanguagePageTitle=应用语言
chinesesimplified.AppLanguagePageDescription=选择安装完成后 FastZIP 默认使用的语言。
chinesesimplified.AppLanguagePrompt=FastZIP 主界面和消息窗口的默认语言：
chinesesimplified.LanguageOptionEnglish=English
chinesesimplified.LanguageOptionChineseSimplified=简体中文
japanese.TaskDesktopIcon=デスクトップ ショートカットを作成
japanese.TaskFileAssociations=書庫の関連付けと「プログラムから開く」を登録
japanese.TaskDefaultApps=セットアップ後に Windows の既定のアプリを開く
japanese.TaskContextMenu=エクスプローラーの圧縮コンテキストメニューを追加
japanese.TaskAutostart=Windows サインイン時に FastZIP を起動
japanese.LaunchFastZIP=FastZIP を起動
japanese.ArchiveTypeName=FastZIP 書庫
japanese.ApplicationDescription=FastZIP 書庫マネージャー
japanese.ContextMenuRootVerb=FastZIP で圧縮
japanese.AppLanguagePageTitle=アプリの言語
japanese.AppLanguagePageDescription=インストール後に FastZIP が既定で使用する言語を選択します。
japanese.AppLanguagePrompt=FastZIP アプリとメッセージウィンドウの既定の言語:
korean.TaskDesktopIcon=바탕 화면 바로 가기 만들기
korean.TaskFileAssociations=압축 파일 연결과 "연결 프로그램" 항목 등록
korean.TaskDefaultApps=설치 후 Windows 기본 앱 열기
korean.TaskContextMenu=탐색기 압축 컨텍스트 메뉴 추가
korean.TaskAutostart=Windows 로그인 시 FastZIP 시작
korean.LaunchFastZIP=FastZIP 실행
korean.ArchiveTypeName=FastZIP 압축 파일
korean.ApplicationDescription=FastZIP 압축 파일 관리자
korean.ContextMenuRootVerb=FastZIP으로 압축
korean.AppLanguagePageTitle=앱 언어
korean.AppLanguagePageDescription=설치 후 FastZIP에서 기본으로 사용할 언어를 선택합니다.
korean.AppLanguagePrompt=FastZIP 앱과 메시지 창의 기본 언어:
french.TaskDesktopIcon=Créer un raccourci sur le Bureau
french.TaskFileAssociations=Enregistrer les associations d'archives et les entrées Ouvrir avec
french.TaskDefaultApps=Ouvrir les applications par défaut de Windows après l'installation
french.TaskContextMenu=Ajouter le menu contextuel de compression dans l'Explorateur
french.TaskAutostart=Lancer FastZIP à l'ouverture de session Windows
french.LaunchFastZIP=Lancer FastZIP
french.ArchiveTypeName=Archive FastZIP
french.ApplicationDescription=Gestionnaire d'archives FastZIP
french.ContextMenuRootVerb=Compresser avec FastZIP vers
french.AppLanguagePageTitle=Langue de l'application
french.AppLanguagePageDescription=Choisissez la langue par défaut utilisée par FastZIP après l'installation.
french.AppLanguagePrompt=Langue par défaut de l'application FastZIP et des fenêtres de message :
german.TaskDesktopIcon=Desktopverknüpfung erstellen
german.TaskFileAssociations=Archivzuordnungen und "Öffnen mit"-Einträge registrieren
german.TaskDefaultApps=Nach der Installation Windows-Standard-Apps öffnen
german.TaskContextMenu=Komprimierungs-Kontextmenü im Explorer hinzufügen
german.TaskAutostart=FastZIP bei der Windows-Anmeldung starten
german.LaunchFastZIP=FastZIP starten
german.ArchiveTypeName=FastZIP-Archiv
german.ApplicationDescription=FastZIP-Archivmanager
german.ContextMenuRootVerb=Mit FastZIP komprimieren nach
german.AppLanguagePageTitle=Anwendungssprache
german.AppLanguagePageDescription=Wählen Sie die Sprache, die FastZIP nach der Installation standardmäßig verwendet.
german.AppLanguagePrompt=Standardsprache für die FastZIP-App und Meldungsfenster:
spanish.TaskDesktopIcon=Crear un acceso directo en el escritorio
spanish.TaskFileAssociations=Registrar asociaciones de archivos y entradas Abrir con
spanish.TaskDefaultApps=Abrir Aplicaciones predeterminadas de Windows después de la instalación
spanish.TaskContextMenu=Agregar el menú contextual de compresión del Explorador
spanish.TaskAutostart=Iniciar FastZIP al iniciar sesión en Windows
spanish.LaunchFastZIP=Iniciar FastZIP
spanish.ArchiveTypeName=Archivo FastZIP
spanish.ApplicationDescription=Administrador de archivos FastZIP
spanish.ContextMenuRootVerb=Comprimir con FastZIP a
spanish.AppLanguagePageTitle=Idioma de la aplicación
spanish.AppLanguagePageDescription=Elija el idioma predeterminado que FastZIP usará después de la instalación.
spanish.AppLanguagePrompt=Idioma predeterminado de la aplicación FastZIP y de las ventanas de mensaje:
italian.TaskDesktopIcon=Creare un collegamento sul desktop
italian.TaskFileAssociations=Registrare le associazioni degli archivi e le voci Apri con
italian.TaskDefaultApps=Aprire le App predefinite di Windows dopo l'installazione
italian.TaskContextMenu=Aggiungere il menu contestuale di compressione di Esplora file
italian.TaskAutostart=Avviare FastZIP all'accesso a Windows
italian.LaunchFastZIP=Avvia FastZIP
italian.ArchiveTypeName=Archivio FastZIP
italian.ApplicationDescription=Gestore archivi FastZIP
italian.ContextMenuRootVerb=Comprimi con FastZIP in
italian.AppLanguagePageTitle=Lingua dell'applicazione
italian.AppLanguagePageDescription=Scegli la lingua predefinita usata da FastZIP dopo l'installazione.
italian.AppLanguagePrompt=Lingua predefinita dell'app FastZIP e delle finestre di messaggio:
brazilianportuguese.TaskDesktopIcon=Criar atalho na área de trabalho
brazilianportuguese.TaskFileAssociations=Registrar associações de arquivos e entradas Abrir com
brazilianportuguese.TaskDefaultApps=Abrir Aplicativos padrão do Windows após a instalação
brazilianportuguese.TaskContextMenu=Adicionar o menu de contexto de compactação do Explorador
brazilianportuguese.TaskAutostart=Iniciar o FastZIP quando eu entrar no Windows
brazilianportuguese.LaunchFastZIP=Iniciar FastZIP
brazilianportuguese.ArchiveTypeName=Arquivo FastZIP
brazilianportuguese.ApplicationDescription=Gerenciador de arquivos FastZIP
brazilianportuguese.ContextMenuRootVerb=Compactar com FastZIP para
brazilianportuguese.AppLanguagePageTitle=Idioma do aplicativo
brazilianportuguese.AppLanguagePageDescription=Escolha o idioma padrão usado pelo FastZIP após a instalação.
brazilianportuguese.AppLanguagePrompt=Idioma padrão do aplicativo FastZIP e das janelas de mensagem:
russian.TaskDesktopIcon=Создать ярлык на рабочем столе
russian.TaskFileAssociations=Зарегистрировать ассоциации архивов и пункты "Открыть с помощью"
russian.TaskDefaultApps=Открыть приложения по умолчанию Windows после установки
russian.TaskContextMenu=Добавить контекстное меню сжатия в Проводник
russian.TaskAutostart=Запускать FastZIP при входе в Windows
russian.LaunchFastZIP=Запустить FastZIP
russian.ArchiveTypeName=Архив FastZIP
russian.ApplicationDescription=Менеджер архивов FastZIP
russian.ContextMenuRootVerb=Сжать с FastZIP в
russian.AppLanguagePageTitle=Язык приложения
russian.AppLanguagePageDescription=Выберите язык, который FastZIP будет использовать по умолчанию после установки.
russian.AppLanguagePrompt=Язык по умолчанию для приложения FastZIP и окон сообщений:
arabic.TaskDesktopIcon=إنشاء اختصار على سطح المكتب
arabic.TaskFileAssociations=تسجيل اقترانات الأرشيف وخيارات "فتح باستخدام"
arabic.TaskDefaultApps=فتح تطبيقات Windows الافتراضية بعد التثبيت
arabic.TaskContextMenu=إضافة قائمة الضغط إلى قائمة مستكشف الملفات
arabic.TaskAutostart=تشغيل FastZIP عند تسجيل الدخول إلى Windows
arabic.LaunchFastZIP=تشغيل FastZIP
arabic.ArchiveTypeName=أرشيف FastZIP
arabic.ApplicationDescription=مدير أرشيف FastZIP
arabic.ContextMenuRootVerb=ضغط باستخدام FastZIP إلى
arabic.AppLanguagePageTitle=لغة التطبيق
arabic.AppLanguagePageDescription=اختر اللغة الافتراضية التي سيستخدمها FastZIP بعد التثبيت.
arabic.AppLanguagePrompt=اللغة الافتراضية لتطبيق FastZIP ونوافذ الرسائل:
turkish.TaskDesktopIcon=Masaüstü kısayolu oluştur
turkish.TaskFileAssociations=Arşiv ilişkilendirmelerini ve Birlikte Aç girdilerini kaydet
turkish.TaskDefaultApps=Kurulumdan sonra Windows Varsayılan Uygulamalarını aç
turkish.TaskContextMenu=Gezgin sıkıştırma sağ tık menüsünü ekle
turkish.TaskAutostart=Windows oturum açıldığında FastZIP'i başlat
turkish.LaunchFastZIP=FastZIP'i başlat
turkish.ArchiveTypeName=FastZIP Arşivi
turkish.ApplicationDescription=FastZIP arşiv yöneticisi
turkish.ContextMenuRootVerb=FastZIP ile şuraya sıkıştır
turkish.AppLanguagePageTitle=Uygulama Dili
turkish.AppLanguagePageDescription=Kurulumdan sonra FastZIP'in varsayılan olarak kullanacağı dili seçin.
turkish.AppLanguagePrompt=FastZIP uygulaması ve ileti pencereleri için varsayılan dil:

[Code]
var
  AppLanguagePage: TWizardPage;
  AppLanguageLabel: TNewStaticText;
  AppLanguageComboBox: TNewComboBox;

function DetectDefaultAppLanguageCode: string;
begin
  if ActiveLanguage = 'chinesesimplified' then
    Result := 'zh-CN'
  else if ActiveLanguage = 'japanese' then
    Result := 'ja'
  else if ActiveLanguage = 'korean' then
    Result := 'ko'
  else if ActiveLanguage = 'french' then
    Result := 'fr'
  else if ActiveLanguage = 'german' then
    Result := 'de'
  else if ActiveLanguage = 'spanish' then
    Result := 'es'
  else if ActiveLanguage = 'italian' then
    Result := 'it'
  else if ActiveLanguage = 'brazilianportuguese' then
    Result := 'pt-BR'
  else if ActiveLanguage = 'russian' then
    Result := 'ru'
  else if ActiveLanguage = 'arabic' then
    Result := 'ar'
  else if ActiveLanguage = 'turkish' then
    Result := 'tr'
  else
    Result := 'en';
end;

function AppLanguageIndexFromCode(const Code: string): Integer;
begin
  if Code = 'zh-CN' then
    Result := 1
  else if Code = 'ja' then
    Result := 2
  else if Code = 'ko' then
    Result := 3
  else if Code = 'fr' then
    Result := 4
  else if Code = 'de' then
    Result := 5
  else if Code = 'es' then
    Result := 6
  else if Code = 'it' then
    Result := 7
  else if Code = 'pt-BR' then
    Result := 8
  else if Code = 'ru' then
    Result := 9
  else if Code = 'ar' then
    Result := 10
  else if Code = 'tr' then
    Result := 11
  else
    Result := 0;
end;

function GetSelectedAppLanguageCode: string;
begin
  if AppLanguageComboBox = nil then
  begin
    Result := DetectDefaultAppLanguageCode;
    exit;
  end;

  case AppLanguageComboBox.ItemIndex of
    1: Result := 'zh-CN';
    2: Result := 'ja';
    3: Result := 'ko';
    4: Result := 'fr';
    5: Result := 'de';
    6: Result := 'es';
    7: Result := 'it';
    8: Result := 'pt-BR';
    9: Result := 'ru';
    10: Result := 'ar';
    11: Result := 'tr';
  else
    Result := 'en';
  end;
end;

function GetSettingsRootDir: string;
begin
  if IsAdminInstallMode then
    Result := ExpandConstant('{commonappdata}\FastZIP')
  else
    Result := ExpandConstant('{localappdata}\FastZIP');
end;

procedure SaveSelectedAppLanguagePreference;
var
  SettingsDir: string;
  SettingsPath: string;
begin
  SettingsDir := GetSettingsRootDir;
  if not DirExists(SettingsDir) then
  begin
    if not ForceDirectories(SettingsDir) then
    begin
      Log('Failed to create settings directory: ' + SettingsDir);
      exit;
    end;
  end;

  SettingsPath := AddBackslash(SettingsDir) + 'settings.ini';
  SetIniString('ui', 'language', GetSelectedAppLanguageCode, SettingsPath);
end;

procedure InitializeWizard;
begin
  AppLanguagePage := CreateCustomPage(
    wpSelectTasks,
    ExpandConstant('{cm:AppLanguagePageTitle}'),
    ExpandConstant('{cm:AppLanguagePageDescription}')
  );

  AppLanguageLabel := TNewStaticText.Create(AppLanguagePage);
  AppLanguageLabel.Parent := AppLanguagePage.Surface;
  AppLanguageLabel.Left := 0;
  AppLanguageLabel.Top := 0;
  AppLanguageLabel.Width := AppLanguagePage.SurfaceWidth;
  AppLanguageLabel.Height := ScaleY(36);
  AppLanguageLabel.AutoSize := False;
  AppLanguageLabel.WordWrap := True;
  AppLanguageLabel.Caption := ExpandConstant('{cm:AppLanguagePrompt}');

  AppLanguageComboBox := TNewComboBox.Create(AppLanguagePage);
  AppLanguageComboBox.Parent := AppLanguagePage.Surface;
  AppLanguageComboBox.Left := 0;
  AppLanguageComboBox.Top := AppLanguageLabel.Top + AppLanguageLabel.Height + ScaleY(10);
  AppLanguageComboBox.Width := ScaleX(240);
  AppLanguageComboBox.Style := csDropDownList;
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionEnglish}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionChineseSimplified}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionJapanese}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionKorean}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionFrench}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionGerman}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionSpanish}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionItalian}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionPortugueseBrazil}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionRussian}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionArabic}'));
  AppLanguageComboBox.Items.Add(ExpandConstant('{cm:LanguageOptionTurkish}'));
  AppLanguageComboBox.DropDownCount := 12;
  AppLanguageComboBox.ItemIndex := AppLanguageIndexFromCode(DetectDefaultAppLanguageCode);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    SaveSelectedAppLanguagePreference;
end;
