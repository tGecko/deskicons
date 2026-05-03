#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::collapsible_if)]

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString, c_void};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::iter::once;
use std::mem::{self};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError, HANDLE,
    HINSTANCE, HWND, LPARAM, LRESULT, MAX_PATH, POINT, RPC_E_CHANGED_MODE, S_FALSE, S_OK,
    WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::Graphics::GdiPlus::{
    GdipCreateBitmapFromFile, GdipCreateHICONFromBitmap, GdipDisposeImage, GdiplusShutdown,
    GdiplusStartup, GdiplusStartupInput, GpBitmap, GpImage, Ok as GDIP_OK,
};
use windows::Win32::Storage::FileSystem::SYNCHRONIZE;
use windows::Win32::Storage::FileSystem::{
    CopyFileW, DeleteFileW, FILE_TYPE_UNKNOWN, GetFileType, MOVEFILE_COPY_ALLOWED,
    MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile, IServiceProvider,
};
use windows::Win32::System::Console::AttachConsole;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_SET_VALUE, REG_DWORD, REG_NOTIFY_CHANGE_LAST_SET,
    REG_OPEN_CREATE_OPTIONS, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW, RegGetValueW,
    RegNotifyChangeKeyValue, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    CREATE_NEW_CONSOLE, CreateEventW, CreateMutexW, CreateProcessW, GetCurrentProcessId,
    OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_INFORMATION, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW, STARTUPINFOW,
    SetEvent, Sleep, TerminateProcess, WaitForMultipleObjects, WaitForSingleObject,
};
use windows::Win32::System::Variant::{VARIANT, VT_I4, VariantInit};
use windows::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION, TaskDialogIndirect,
};
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::StrRetToBufW;
use windows::Win32::UI::Shell::{
    CSIDL_DESKTOP, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop, FWF_AUTOARRANGE,
    IEnumIDList, IFolderView, IFolderView2, IShellBrowser, IShellFolder, IShellLinkW, IShellView,
    IShellWindows, KNOWN_FOLDER_FLAG, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD,
    NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
    SHCNE_ASSOCCHANGED, SHCNE_UPDATEDIR, SHCNF_IDLIST, SHCNF_PATHW, SHChangeNotify,
    SHGDN_FORPARSING, SHGetKnownFolderPath, SID_STopLevelBrowser, SVGIO_ALLVIEW, SVSI_POSITIONITEM,
    SWC_DESKTOP, SWFO_NEEDDISPATCH, Shell_NotifyIconW, ShellExecuteW, ShellLink, ShellWindows,
};
use windows::Win32::UI::WindowsAndMessaging::HICON;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CREATESTRUCTW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW, GWLP_USERDATA, GetCursorPos,
    GetMessageW, GetWindowLongPtrW, HMENU, IDCANCEL, IDI_APPLICATION, IDOK, LoadIconW,
    MB_DEFBUTTON1, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK,
    MB_OKCANCEL, MESSAGEBOX_STYLE, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, MessageBoxW,
    PostMessageW, PostQuitMessage, RegisterClassW, SW_SHOWNORMAL, SetForegroundWindow,
    SetWindowLongPtrW, TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
    WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK,
    WM_NCCREATE, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_THEMECHANGED, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{BOOL, Error as WinError, GUID, Interface, PCWSTR, PWSTR, w};

type Result<T> = std::result::Result<T, AppError>;
type ItemIdChild = ITEMIDLIST;

const APP_VERSION: &str = "0.1.7";
const WM_DESKICONS_TRAY: u32 = WM_APP + 1;
const WM_DESKICONS_VD_CHANGED: u32 = WM_APP + 2;
const TRAY_CLASS: PCWSTR = w!("DeskIconsTrayWindow");
const RUN_VALUE_NAME: PCWSTR = w!("DeskIcons");
const IDI_APP_ICON: usize = 1;
const ID_TRAY_ENABLE: u16 = 2001;
const ID_TRAY_ADOPT: u16 = 2002;
const ID_TRAY_RESTORE: u16 = 2003;
const ID_TRAY_RECOVER: u16 = 2004;
const ID_TRAY_OPEN_STATE: u16 = 2005;
const ID_TRAY_STARTUP: u16 = 2006;
const ID_TRAY_EXIT: u16 = 2007;
const ID_TRAY_LANG_BASE: u16 = 3000;

#[derive(Debug)]
struct AppError(String);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<WinError> for AppError {
    fn from(value: WinError) -> Self {
        Self(value.message())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    En,
    De,
}

#[derive(Clone, Copy)]
struct Strings {
    install_title: &'static str,
    install_instruction: &'static str,
    install_content: &'static str,
    install_button: &'static str,
    install_start_with_windows: &'static str,
    install_fallback_content: &'static str,
    install_copy_error_prefix: &'static str,
    install_copy_error_title: &'static str,
    install_started_error: &'static str,
    update_title: &'static str,
    update_instruction: &'static str,
    update_content: &'static str,
    update_button: &'static str,
    update_fallback_content: &'static str,
    cancel_button: &'static str,
    menu_enabled: &'static str,
    menu_adopt: &'static str,
    menu_restore_layout: &'static str,
    menu_recover: &'static str,
    menu_open_state: &'static str,
    menu_startup: &'static str,
    menu_language: &'static str,
    menu_exit: &'static str,
    msg_adopt_ok: &'static str,
    msg_adopt_error: &'static str,
    msg_recover_nothing: &'static str,
    msg_title: &'static str,
    msg_error_title: &'static str,
    notif_active_title: &'static str,
    notif_installed: &'static str,
    notif_adopted: &'static str,
}

static STRINGS_EN: Strings = Strings {
    install_title: "Install DeskIcons",
    install_instruction: "Install DeskIcons for this user?",
    install_content: "DeskIcons needs to be installed under your local application data folder before it runs in the tray.",
    install_button: "Install",
    install_start_with_windows: "Start automatically with Windows",
    install_fallback_content: "Install DeskIcons under your local application data folder?",
    install_copy_error_prefix: "DeskIcons could not be installed:\n\n",
    install_copy_error_title: "Install DeskIcons",
    install_started_error: "DeskIcons was installed, but the installed copy could not be started.",
    update_title: "Update DeskIcons",
    update_instruction: "Update DeskIcons?",
    update_content: "Click Update to install the new version.",
    update_button: "Update",
    update_fallback_content: "Update DeskIcons in your local application data folder?",
    cancel_button: "Cancel",
    menu_enabled: "Enabled",
    menu_adopt: "Adopt Current Desktop",
    menu_restore_layout: "Restore Layout",
    menu_recover: "Recover Interrupted Swap",
    menu_open_state: "Open State Folder",
    menu_startup: "Start with Windows",
    menu_language: "Language",
    menu_exit: "Exit",
    msg_adopt_ok: "Current user Desktop adopted. Public Desktop icons remain unmanaged.",
    msg_adopt_error: "Could not determine the current virtual desktop.",
    msg_recover_nothing: "No interrupted swap journal exists.",
    msg_title: "DeskIcons",
    msg_error_title: "DeskIcons Error",
    notif_active_title: "DeskIcons is now active",
    notif_installed: "DeskIcons was installed and is running in the tray.",
    notif_adopted: "The current user Desktop was adopted for this virtual desktop. Public Desktop icons are unmanaged.",
};

static STRINGS_DE: Strings = Strings {
    install_title: "DeskIcons installieren",
    install_instruction: "DeskIcons für diesen Benutzer installieren?",
    install_content: "DeskIcons muss in Ihrem lokalen App-Daten-Ordner installiert werden, bevor es im Infobereich ausgeführt werden kann.",
    install_button: "Installieren",
    install_start_with_windows: "Automatisch mit Windows starten",
    install_fallback_content: "DeskIcons im lokalen App-Daten-Ordner installieren?",
    install_copy_error_prefix: "DeskIcons konnte nicht installiert werden:\n\n",
    install_copy_error_title: "DeskIcons installieren",
    install_started_error: "DeskIcons wurde installiert, aber die installierte Version konnte nicht gestartet werden.",
    update_title: "DeskIcons aktualisieren",
    update_instruction: "DeskIcons aktualisieren?",
    update_content: "Klicken Sie auf 'Aktualisieren', um die neue Version zu installieren.",
    update_button: "Aktualisieren",
    update_fallback_content: "DeskIcons im lokalen App-Daten-Ordner aktualisieren?",
    cancel_button: "Abbrechen",
    menu_enabled: "Aktiviert",
    menu_adopt: "Aktuellen Desktop übernehmen",
    menu_restore_layout: "Layout wiederherstellen",
    menu_recover: "Unterbrochenen Tausch fortsetzen",
    menu_open_state: "Statusordner öffnen",
    menu_startup: "Mit Windows starten",
    menu_language: "Sprache",
    menu_exit: "Beenden",
    msg_adopt_ok: "Der Desktop des aktuellen Benutzers wurde übernommen. Öffentliche Desktop-Icons bleiben nicht verwaltet.",
    msg_adopt_error: "Der aktuelle virtuelle Desktop konnte nicht ermittelt werden.",
    msg_recover_nothing: "Kein unterbrochenes Tausch-Journal vorhanden.",
    msg_title: "DeskIcons",
    msg_error_title: "DeskIcons Fehler",
    notif_active_title: "DeskIcons ist jetzt aktiv",
    notif_installed: "DeskIcons wurde installiert und läuft im Infobereich.",
    notif_adopted: "Der aktuelle Desktop wurde für diesen virtual Desktop übernommen. Öffentliche Desktop-Icons sind nicht verwaltet.",
};

static LANGUAGE: OnceLock<std::sync::Mutex<Language>> = OnceLock::new();

fn set_language(lang: Language) {
    *LANGUAGE
        .get_or_init(|| std::sync::Mutex::new(Language::En))
        .lock()
        .unwrap() = lang;
}

fn lang() -> Language {
    *LANGUAGE
        .get_or_init(|| std::sync::Mutex::new(Language::En))
        .lock()
        .unwrap()
}

fn s() -> &'static Strings {
    match lang() {
        Language::En => &STRINGS_EN,
        Language::De => &STRINGS_DE,
    }
}

fn language_code(lang: Language) -> &'static str {
    match lang {
        Language::En => "en",
        Language::De => "de",
    }
}

fn language_from_code(code: &str) -> Language {
    if code == "de" {
        Language::De
    } else {
        Language::En
    }
}

fn detect_system_language() -> Language {
    let primary = unsafe { GetUserDefaultUILanguage() } & 0x03ff;
    if primary == 0x07 {
        Language::De
    } else {
        Language::En
    }
}

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(once(0)).collect()
}

fn wide_str(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(once(0)).collect()
}

fn copy_wide_truncated(dest: &mut [u16], src: &[u16]) {
    if dest.is_empty() {
        return;
    }
    let n = src
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(src.len())
        .min(dest.len() - 1);
    dest[..n].copy_from_slice(&src[..n]);
    dest[n] = 0;
}

fn os_string_from_wide_z(buffer: &[u16]) -> OsString {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    OsString::from_wide(&buffer[..len])
}

fn path_display(path: &Path) -> String {
    path.as_os_str().to_string_lossy().into_owned()
}

fn last_error(what: &str) -> AppError {
    AppError(format!(
        "{what}: {}",
        unsafe { GetLastError() }.to_hresult().message()
    ))
}

fn check_bool(ok: BOOL, what: &str) -> Result<()> {
    if ok.as_bool() {
        Ok(())
    } else {
        Err(last_error(what))
    }
}

struct CoApartment {
    initialized: bool,
}

impl CoApartment {
    fn init() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() {
            if hr != RPC_E_CHANGED_MODE {
                return Err(WinError::from_hresult(hr).into());
            }
            return Ok(Self { initialized: false });
        }
        Ok(Self { initialized: true })
    }
}

impl Drop for CoApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

struct Handle(HANDLE);

impl Handle {
    fn new(handle: HANDLE) -> Option<Self> {
        if handle.is_invalid() || handle.0.is_null() {
            None
        } else {
            Some(Self(handle))
        }
    }

    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct RegKey(HKEY);

impl RegKey {
    fn raw(&self) -> HKEY {
        self.0
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

struct OwnedMenu(HMENU);

impl Drop for OwnedMenu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}

struct OwnedIcon(HICON);

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}

#[derive(Clone)]
struct Paths {
    desktop: PathBuf,
    public_desktop: PathBuf,
    root: PathBuf,
    sets: PathBuf,
    layouts: PathBuf,
    logs: PathBuf,
    exports: PathBuf,
    config_file: PathBuf,
    journal_file: PathBuf,
    active_file: PathBuf,
    disabled_file: PathBuf,
    install_notice_file: PathBuf,
    start_menu_shortcut: PathBuf,
}

impl Paths {
    fn set_files(&self, guid: &str) -> PathBuf {
        self.sets.join(guid).join("files")
    }

    fn layout_file(&self, guid: &str) -> PathBuf {
        self.layouts.join(format!("{guid}.tsv"))
    }
}

fn known_folder_path(id: &GUID) -> Result<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None)?;
        let mut len = 0usize;
        while *raw.0.add(len) != 0 {
            len += 1;
        }
        let value = OsString::from_wide(std::slice::from_raw_parts(raw.0, len));
        CoTaskMemFree(Some(raw.0 as *const c_void));
        Ok(PathBuf::from(value))
    }
}

fn paths() -> Result<Paths> {
    let _co = CoApartment::init()?;
    let root = PathBuf::from(env::var_os("LOCALAPPDATA").ok_or_else(|| {
        AppError("Required environment variable is missing: LOCALAPPDATA".into())
    })?)
    .join("DeskIcons");
    Ok(Paths {
        desktop: known_folder_path(&FOLDERID_Desktop)?,
        public_desktop: known_folder_path(&FOLDERID_PublicDesktop)?,
        sets: root.join("sets"),
        layouts: root.join("layouts"),
        logs: root.join("logs"),
        exports: root.join("exports"),
        config_file: root.join("config.ini"),
        journal_file: root.join("swap.journal"),
        active_file: root.join("active-desktop.txt"),
        disabled_file: root.join("disabled"),
        install_notice_file: root.join("install-notice"),
        start_menu_shortcut: known_folder_path(&FOLDERID_Programs)?.join("DeskIcons.lnk"),
        root,
    })
}

fn ensure_dirs(p: &Paths) -> Result<()> {
    fs::create_dir_all(&p.sets)?;
    fs::create_dir_all(&p.layouts)?;
    fs::create_dir_all(&p.logs)?;
    fs::create_dir_all(&p.exports)?;
    Ok(())
}

fn timestamp_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn log_line(p: &Paths, message: &str) {
    let _ = fs::create_dir_all(&p.logs);
    if let Ok(mut out) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(p.logs.join("deskicons.log"))
    {
        let _ = writeln!(out, "{} {}", timestamp_now(), message);
    }
}

fn log_error(p: &Paths, err: &dyn std::error::Error) {
    log_line(p, &format!("ERROR {err}"));
}

#[derive(Clone)]
struct Config {
    enabled: bool,
    manage_non_shortcuts: bool,
    poll_ms: u64,
    language: Option<Language>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            manage_non_shortcuts: true,
            poll_ms: 750,
            language: None,
        }
    }
}

fn trim_ascii(value: &str) -> &str {
    value.trim_matches(|c: char| c.is_ascii_whitespace())
}

fn load_config(p: &Paths) -> Config {
    let Ok(file) = File::open(&p.config_file) else {
        return Config::default();
    };
    let mut config = Config::default();
    for line in BufReader::new(file)
        .lines()
        .map_while(std::result::Result::ok)
    {
        let trimmed = trim_ascii(&line);
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        match trim_ascii(key) {
            "enabled" => config.enabled = trim_ascii(value) != "0",
            "manage_non_shortcuts" => config.manage_non_shortcuts = trim_ascii(value) != "0",
            "poll_ms" => {
                config.poll_ms = trim_ascii(value)
                    .parse::<u64>()
                    .unwrap_or(750)
                    .clamp(250, 10_000);
            }
            "language" => config.language = Some(language_from_code(trim_ascii(value))),
            _ => {}
        }
    }
    config
}

fn save_config(p: &Paths, config: &Config) -> Result<()> {
    fs::create_dir_all(&p.root)?;
    let mut out = File::create(&p.config_file)?;
    writeln!(out, "enabled={}", if config.enabled { 1 } else { 0 })?;
    writeln!(
        out,
        "manage_non_shortcuts={}",
        if config.manage_non_shortcuts { 1 } else { 0 }
    )?;
    writeln!(out, "poll_ms={}", config.poll_ms)?;
    if let Some(language) = config.language {
        writeln!(out, "language={}", language_code(language))?;
    }
    Ok(())
}

fn app_enabled(p: &Paths) -> bool {
    load_config(p).enabled && !p.disabled_file.exists()
}

fn set_enabled(p: &Paths, enabled: bool) -> Result<()> {
    let mut config = load_config(p);
    config.enabled = enabled;
    save_config(p, &config)?;
    if enabled {
        let _ = fs::remove_file(&p.disabled_file);
    } else {
        fs::create_dir_all(&p.root)?;
        fs::write(&p.disabled_file, b"disabled\n")?;
    }
    log_line(
        p,
        if enabled {
            "enabled agent"
        } else {
            "disabled agent"
        },
    );
    Ok(())
}

fn read_text_file_trimmed(path: &Path) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let trimmed = trim_ascii(&value).to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn write_text_file(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{value}\n"))?;
    Ok(())
}

fn is_skipped_desktop_entry(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("desktop.ini"))
}

fn child_entries(dir: &Path, log_paths: Option<&Paths>) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let Ok(read_dir) = fs::read_dir(dir) else {
        return entries;
    };
    for entry in read_dir {
        match entry {
            Ok(entry) if !is_skipped_desktop_entry(&entry.path()) => entries.push(entry.path()),
            Ok(_) => {}
            Err(err) => {
                if let Some(p) = log_paths {
                    log_line(
                        p,
                        &format!("could not enumerate {}: {err}", path_display(dir)),
                    );
                }
            }
        }
    }
    entries.sort();
    entries
}

fn should_manage_entry(path: &Path, config: &Config) -> bool {
    config.manage_non_shortcuts
        || path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("lnk"))
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'.'
            | b'-'
            | b'_'
            | b' '
            | b'\\'
            | b'/'
            | b':' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn percent_decode(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

fn encode_path(path: &Path) -> String {
    percent_encode(&path_display(path))
}

fn decode_path(value: &str) -> PathBuf {
    PathBuf::from(percent_decode(value))
}

#[derive(Clone)]
struct PlannedMove {
    from: PathBuf,
    to: PathBuf,
}

struct Journal {
    stage: String,
    from_guid: String,
    to_guid: String,
    outbound: Vec<PlannedMove>,
    inbound: Vec<PlannedMove>,
}

fn write_journal(p: &Paths, mut journal: Journal, stage: &str) -> Result<()> {
    journal.stage = stage.to_string();
    fs::create_dir_all(&p.root)?;
    let mut out = File::create(&p.journal_file)?;
    writeln!(out, "version\t1")?;
    writeln!(out, "stage\t{}", journal.stage)?;
    writeln!(out, "from\t{}", journal.from_guid)?;
    writeln!(out, "to\t{}", journal.to_guid)?;
    for mv in &journal.outbound {
        writeln!(
            out,
            "out\t{}\t{}",
            encode_path(&mv.from),
            encode_path(&mv.to)
        )?;
    }
    for mv in &journal.inbound {
        writeln!(
            out,
            "in\t{}\t{}",
            encode_path(&mv.from),
            encode_path(&mv.to)
        )?;
    }
    Ok(())
}

fn read_journal(p: &Paths) -> Result<Option<Journal>> {
    let Ok(file) = File::open(&p.journal_file) else {
        return Ok(None);
    };
    let mut journal = Journal {
        stage: String::new(),
        from_guid: String::new(),
        to_guid: String::new(),
        outbound: Vec::new(),
        inbound: Vec::new(),
    };
    for line in BufReader::new(file).lines() {
        let line = line?;
        let parts: Vec<_> = line.split('\t').collect();
        match parts.as_slice() {
            ["stage", value] => journal.stage = (*value).to_string(),
            ["from", value] => journal.from_guid = (*value).to_string(),
            ["to", value] => journal.to_guid = (*value).to_string(),
            ["out", from, to] => journal.outbound.push(PlannedMove {
                from: decode_path(from),
                to: decode_path(to),
            }),
            ["in", from, to] => journal.inbound.push(PlannedMove {
                from: decode_path(from),
                to: decode_path(to),
            }),
            _ => {}
        }
    }
    if journal.stage.is_empty() {
        journal.stage = "planned".to_string();
    }
    if journal.from_guid.is_empty() || journal.to_guid.is_empty() {
        return Err(AppError("Swap journal is malformed".into()));
    }
    Ok(Some(journal))
}

fn clear_journal(p: &Paths) {
    let _ = fs::remove_file(&p.journal_file);
}

fn validate_moves(moves: &[PlannedMove]) -> Result<()> {
    for mv in moves {
        if !mv.from.exists() {
            return Err(AppError(format!(
                "Source disappeared before move: {}",
                path_display(&mv.from)
            )));
        }
        if mv.to.exists() {
            return Err(AppError(format!(
                "Refusing to overwrite existing path: {}",
                path_display(&mv.to)
            )));
        }
    }
    Ok(())
}

fn move_path(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if from.is_dir() {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(rename_err) => {
                copy_dir_all(from, to).map_err(|copy_err| {
                    AppError(format!(
                        "Could not copy directory {} -> {} after rename failed ({rename_err}): {copy_err}",
                        path_display(from),
                        path_display(to)
                    ))
                })?;
                fs::remove_dir_all(from).map_err(|remove_err| {
                    AppError(format!(
                        "Copied directory to {}, but could not remove original {}: {remove_err}",
                        path_display(to),
                        path_display(from)
                    ))
                })?;
                return Ok(());
            }
        }
    }
    let from_w = wide(from.as_os_str());
    let to_w = wide(to.as_os_str());
    unsafe {
        MoveFileExW(
            PCWSTR(from_w.as_ptr()),
            PCWSTR(to_w.as_ptr()),
            MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|_| {
            last_error(&format!(
                "MoveFileExW {} -> {}",
                path_display(from),
                path_display(to)
            ))
        })
    }
}

fn move_completed_or_finish(mv: &PlannedMove) -> Result<bool> {
    let src_exists = mv.from.exists();
    let dst_exists = mv.to.exists();
    if src_exists && dst_exists {
        return Err(AppError(format!(
            "Recovery conflict: both source and destination exist for {}",
            path_display(&mv.from)
        )));
    }
    if src_exists && !dst_exists {
        move_path(&mv.from, &mv.to)?;
        return Ok(true);
    }
    if !src_exists && !dst_exists {
        return Err(AppError(format!(
            "Recovery lost both source and destination: {} -> {}",
            path_display(&mv.from),
            path_display(&mv.to)
        )));
    }
    Ok(dst_exists)
}

fn finish_move_set(moves: &[PlannedMove]) -> Result<()> {
    for mv in moves {
        move_completed_or_finish(mv)?;
    }
    Ok(())
}

fn rollback_move_set(moves: &[PlannedMove]) -> Result<()> {
    for mv in moves.iter().rev() {
        move_completed_or_finish(&PlannedMove {
            from: mv.to.clone(),
            to: mv.from.clone(),
        })?;
    }
    Ok(())
}

fn recover_journal(p: &Paths, verbose: bool) -> Result<bool> {
    let Some(journal) = read_journal(p)? else {
        return Ok(false);
    };
    if verbose {
        println!("Recovering interrupted swap");
        println!("  stage: {}", journal.stage);
        println!("  from:  {}", journal.from_guid);
        println!("  to:    {}", journal.to_guid);
    }
    log_line(
        p,
        &format!(
            "recovering interrupted swap stage={} {} -> {}",
            journal.stage, journal.from_guid, journal.to_guid
        ),
    );
    match journal.stage.as_str() {
        "planned" => {
            rollback_move_set(&journal.outbound)?;
            set_active_desktop(p, &journal.from_guid)?;
        }
        "outbound-complete" | "inbound-complete" => {
            finish_move_set(&journal.outbound)?;
            finish_move_set(&journal.inbound)?;
            set_active_desktop(p, &journal.to_guid)?;
            refresh_desktop(p);
            restore_layout(p, &journal.to_guid, verbose)?;
        }
        "rollback" => {
            rollback_move_set(&journal.inbound)?;
            rollback_move_set(&journal.outbound)?;
            set_active_desktop(p, &journal.from_guid)?;
        }
        _ => {
            return Err(AppError(format!(
                "Swap journal has unknown stage: {}",
                journal.stage
            )));
        }
    }
    clear_journal(p);
    refresh_desktop(p);
    log_line(p, "recovered interrupted swap");
    Ok(true)
}

fn guid_to_string(guid: &GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7]
    )
}

fn reg_get_binary(root: HKEY, subkey: PCWSTR, value_name: PCWSTR) -> Option<Vec<u8>> {
    let mut ty = Default::default();
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            root,
            subkey,
            value_name,
            RRF_RT_REG_BINARY,
            Some(&mut ty),
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    let status = unsafe {
        RegGetValueW(
            root,
            subkey,
            value_name,
            RRF_RT_REG_BINARY,
            Some(&mut ty),
            Some(data.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        Some(data)
    } else {
        None
    }
}

fn read_guid_value(root: HKEY, subkey: PCWSTR, value_name: PCWSTR) -> Option<GUID> {
    let data = reg_get_binary(root, subkey, value_name)?;
    if data.len() != mem::size_of::<GUID>() {
        return None;
    }
    Some(unsafe { std::ptr::read_unaligned(data.as_ptr() as *const GUID) })
}

fn read_guid_array_value(root: HKEY, subkey: PCWSTR, value_name: PCWSTR) -> Option<Vec<GUID>> {
    let data = reg_get_binary(root, subkey, value_name)?;
    if data.is_empty() || data.len() % mem::size_of::<GUID>() != 0 {
        return None;
    }
    let mut ids = Vec::new();
    for chunk in data.chunks_exact(mem::size_of::<GUID>()) {
        ids.push(unsafe { std::ptr::read_unaligned(chunk.as_ptr() as *const GUID) });
    }
    Some(ids)
}

fn current_virtual_desktop_guid() -> Option<GUID> {
    let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VirtualDesktops");
    if let Some(guid) = read_guid_value(HKEY_CURRENT_USER, key, w!("CurrentVirtualDesktop")) {
        return Some(guid);
    }
    let mut session_id = 0;
    unsafe {
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut session_id).is_ok() {
            let fallback = format!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\SessionInfo\\{session_id}\\VirtualDesktops"
            );
            let fallback_w = wide_str(&fallback);
            return read_guid_value(
                HKEY_CURRENT_USER,
                PCWSTR(fallback_w.as_ptr()),
                w!("CurrentVirtualDesktop"),
            );
        }
    }
    None
}

fn virtual_desktop_ids() -> Vec<GUID> {
    read_guid_array_value(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VirtualDesktops"),
        w!("VirtualDesktopIDs"),
    )
    .unwrap_or_default()
}

struct DesktopViewContext {
    shell_view: IShellView,
    folder_view: IFolderView,
}

fn desktop_view_context() -> Result<DesktopViewContext> {
    unsafe {
        let shell_windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)?;
        let mut vt_loc: VARIANT = VariantInit();
        *vt_loc.Anonymous.Anonymous = windows::Win32::System::Variant::VARIANT_0_0 {
            vt: VT_I4,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            Anonymous: windows::Win32::System::Variant::VARIANT_0_0_0 {
                lVal: CSIDL_DESKTOP as i32,
            },
        };
        let vt_empty: VARIANT = VariantInit();
        let mut hwnd = 0i32;
        let dispatch = shell_windows.FindWindowSW(
            &vt_loc,
            &vt_empty,
            SWC_DESKTOP,
            &mut hwnd,
            SWFO_NEEDDISPATCH,
        )?;
        let provider: IServiceProvider = dispatch.cast()?;
        let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser)?;
        let shell_view = browser.QueryActiveShellView()?;
        let folder_view: IFolderView = shell_view.cast()?;
        Ok(DesktopViewContext {
            shell_view,
            folder_view,
        })
    }
}

fn desktop_folder_view() -> Result<IFolderView> {
    Ok(desktop_view_context()?.folder_view)
}

fn parsing_path_for_item(folder: &IShellFolder, item: *const ItemIdChild) -> Option<PathBuf> {
    unsafe {
        let mut strret = STRRET::default();
        if folder
            .GetDisplayNameOf(item, SHGDN_FORPARSING, &mut strret)
            .is_err()
        {
            return None;
        }
        let mut buffer = vec![0u16; MAX_PATH as usize * 4];
        if StrRetToBufW(&mut strret, Some(item), &mut buffer).is_err() {
            return None;
        }
        Some(PathBuf::from(os_string_from_wide_z(&buffer)))
    }
}

fn normalized_path(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path)
        .ok()
        .or_else(|| path.canonicalize().ok())
        .or_else(|| Some(path.to_path_buf()))
}

fn path_equal_ci(a: &Path, b: &Path) -> bool {
    path_display(a).to_lowercase() == path_display(b).to_lowercase()
}

fn is_under_dir(child: &Path, parent: &Path, strict: bool) -> bool {
    let Some(child) = normalized_path(child) else {
        return false;
    };
    let Some(parent) = normalized_path(parent) else {
        return false;
    };
    if path_equal_ci(&child, &parent) {
        return !strict;
    }
    let child_parts: Vec<_> = child.components().collect();
    let parent_parts: Vec<_> = parent.components().collect();
    child_parts.len() > parent_parts.len()
        && parent_parts.iter().zip(child_parts.iter()).all(|(a, b)| {
            a.as_os_str().to_string_lossy().to_lowercase()
                == b.as_os_str().to_string_lossy().to_lowercase()
        })
}

fn relative_name_for_desktop_item(item_path: &Path, desktop: &Path) -> String {
    let Some(child) = normalized_path(item_path) else {
        return String::new();
    };
    let Some(parent) = normalized_path(desktop) else {
        return String::new();
    };
    if !is_under_dir(&child, &parent, true) {
        return String::new();
    }
    let child_parts: Vec<_> = child.components().collect();
    let parent_parts: Vec<_> = parent.components().collect();
    if child_parts.len() <= parent_parts.len() {
        return String::new();
    }
    let mut rel = PathBuf::new();
    for part in child_parts.iter().skip(parent_parts.len()) {
        rel.push(part.as_os_str());
    }
    path_display(&rel)
}

fn save_layout(p: &Paths, guid: &str) -> Result<()> {
    let _co = CoApartment::init()?;
    unsafe {
        let view = desktop_folder_view()?;
        let folder: IShellFolder = view.GetFolder()?;
        let items: IEnumIDList = view.Items(SVGIO_ALLVIEW)?;
        fs::create_dir_all(&p.layouts)?;
        let mut out = File::create(p.layout_file(guid))?;
        loop {
            let mut fetched = [null_mut()];
            if items.Next(&mut fetched, None) != S_OK {
                break;
            }
            let item: *mut ItemIdChild = fetched[0];
            let item_path = parsing_path_for_item(&folder, item);
            if let Ok(pt) = view.GetItemPosition(item) {
                if let Some(item_path) = item_path {
                    if is_under_dir(&item_path, &p.desktop, true) {
                        let rel = relative_name_for_desktop_item(&item_path, &p.desktop);
                        if !rel.is_empty() {
                            writeln!(out, "{}\t{}\t{}", percent_encode(&rel), pt.x, pt.y)?;
                        }
                    }
                }
            }
            CoTaskMemFree(Some(item as *const c_void));
        }
    }
    Ok(())
}

fn load_layout(path: &Path) -> (BTreeMap<String, POINT>, usize) {
    let mut result = BTreeMap::new();
    let mut skipped = 0;
    let Ok(file) = File::open(path) else {
        return (result, skipped);
    };
    for line in BufReader::new(file)
        .lines()
        .map_while(std::result::Result::ok)
    {
        let parts: Vec<_> = line.split('\t').collect();
        if parts.len() >= 3 {
            if let (Ok(x), Ok(y)) = (parts[1].parse::<i32>(), parts[2].parse::<i32>()) {
                result.insert(percent_decode(parts[0]), POINT { x, y });
            } else {
                skipped += 1;
            }
        } else {
            skipped += 1;
        }
    }
    (result, skipped)
}

fn desktop_folder_view2(view: &IFolderView) -> Option<IFolderView2> {
    view.cast().ok()
}

fn visible_desktop_items(
    view: &IFolderView,
    folder: &IShellFolder,
    p: &Paths,
) -> Result<BTreeMap<String, *mut ItemIdChild>> {
    let mut result = BTreeMap::new();
    unsafe {
        let items: IEnumIDList = view.Items(SVGIO_ALLVIEW)?;
        loop {
            let mut fetched = [null_mut()];
            if items.Next(&mut fetched, None) != S_OK {
                break;
            }
            let item: *mut ItemIdChild = fetched[0];
            let mut keep = false;
            if let Some(item_path) = parsing_path_for_item(folder, item) {
                if is_under_dir(&item_path, &p.desktop, true) {
                    let rel = relative_name_for_desktop_item(&item_path, &p.desktop);
                    if !rel.is_empty() {
                        if let Some(old) = result.insert(rel, item) {
                            CoTaskMemFree(Some(old as *const c_void));
                        }
                        keep = true;
                    }
                }
            }
            if !keep {
                CoTaskMemFree(Some(item as *const c_void));
            }
        }
    }
    Ok(result)
}

fn free_visible_items(items: &mut BTreeMap<String, *mut ItemIdChild>) {
    for (_, item) in std::mem::take(items) {
        unsafe {
            CoTaskMemFree(Some(item as *const c_void));
        }
    }
}

fn wait_for_layout_items(
    shell_view: &IShellView,
    view: &IFolderView,
    folder: &IShellFolder,
    p: &Paths,
    layout: &BTreeMap<String, POINT>,
) -> Result<BTreeMap<String, *mut ItemIdChild>> {
    let start = std::time::Instant::now();
    let mut visible = BTreeMap::new();
    loop {
        unsafe {
            let _ = shell_view.Refresh();
        }
        refresh_desktop(p);
        thread::sleep(Duration::from_millis(100));
        free_visible_items(&mut visible);
        visible = visible_desktop_items(view, folder, p)?;
        let matched = layout
            .keys()
            .filter(|name| visible.contains_key(*name))
            .count();
        if matched == layout.len() || start.elapsed() >= Duration::from_secs(10) {
            return Ok(visible);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn restore_layout(p: &Paths, guid: &str, verbose: bool) -> Result<()> {
    let (layout, skipped_rows) = load_layout(&p.layout_file(guid));
    if skipped_rows > 0 {
        log_line(p, &format!("ignored malformed layout rows: {skipped_rows}"));
    }
    if layout.is_empty() {
        if verbose {
            println!("No saved layout for {guid}");
        }
        return Ok(());
    }
    let _co = CoApartment::init()?;
    unsafe {
        let mut desktop_view = desktop_view_context()?;
        let mut view = desktop_view.folder_view.clone();
        let mut folder: IShellFolder = view.GetFolder()?;
        if let Some(view2) = desktop_folder_view2(&view) {
            if let Ok(flags) = view2.GetCurrentFolderFlags() {
                if (flags & FWF_AUTOARRANGE.0 as u32) != 0 {
                    let _ = view2.SetCurrentFolderFlags(FWF_AUTOARRANGE.0 as u32, 0);
                    if verbose {
                        println!(
                            "Disabled Desktop auto-arrange so saved icon positions can be restored."
                        );
                    }
                }
            }
        }
        let mut visible =
            wait_for_layout_items(&desktop_view.shell_view, &view, &folder, p, &layout)?;
        let initial_matches = layout
            .keys()
            .filter(|rel| visible.contains_key(*rel))
            .count();
        if initial_matches == 0 {
            free_visible_items(&mut visible);
            if verbose {
                println!(
                    "Explorer desktop view had no matching saved items; reacquiring the Shell view and retrying."
                );
            }
            desktop_view = desktop_view_context()?;
            view = desktop_view.folder_view.clone();
            folder = view.GetFolder()?;
            visible = wait_for_layout_items(&desktop_view.shell_view, &view, &folder, p, &layout)?;
        }
        let mut apidls = Vec::new();
        let mut positions = Vec::new();
        let mut missing = Vec::new();
        for (rel, pt) in &layout {
            if let Some(item) = visible.get(rel) {
                apidls.push(*item as *const ITEMIDLIST);
                positions.push(*pt);
            } else {
                missing.push(rel.clone());
            }
        }
        let apply_hr = if apidls.is_empty() {
            S_FALSE
        } else {
            match view.SelectAndPositionItems(
                apidls.len() as u32,
                apidls.as_ptr(),
                Some(positions.as_ptr()),
                SVSI_POSITIONITEM.0 as u32,
            ) {
                std::result::Result::Ok(()) => windows::core::HRESULT(0),
                Err(err) => err.code(),
            }
        };
        free_visible_items(&mut visible);
        refresh_desktop(p);
        if verbose {
            println!(
                "Matched {} of {} saved icon positions for {}",
                apidls.len(),
                layout.len(),
                guid
            );
            if !missing.is_empty() {
                println!("Missing from Explorer desktop view:");
                for item in missing {
                    println!("  {item}");
                }
            }
            if apidls.is_empty() {
                println!(
                    "No positions were applied because no saved items matched the current desktop view."
                );
            } else if apply_hr.is_ok() {
                println!("Applied {} saved icon positions.", apidls.len());
            } else {
                println!(
                    "SelectAndPositionItems failed: HRESULT 0x{:08x}",
                    apply_hr.0 as u32
                );
            }
        }
    }
    Ok(())
}

fn dump_visible_items(p: &Paths) -> Result<()> {
    let _co = CoApartment::init()?;
    unsafe {
        let view = desktop_folder_view()?;
        let folder: IShellFolder = view.GetFolder()?;
        let mut visible = visible_desktop_items(&view, &folder, p)?;
        println!(
            "Explorer desktop view items under managed user Desktop {}:",
            path_display(&p.desktop)
        );
        for (rel, item) in &visible {
            if let Ok(pt) = view.GetItemPosition(*item) {
                println!("  {rel}\t{}\t{}", pt.x, pt.y);
            } else {
                println!("  {rel}\t<no position>");
            }
        }
        free_visible_items(&mut visible);
    }
    Ok(())
}

fn refresh_desktop(p: &Paths) {
    let desktop = wide(p.desktop.as_os_str());
    let public = wide(p.public_desktop.as_os_str());
    unsafe {
        SHChangeNotify(
            SHCNE_UPDATEDIR,
            SHCNF_PATHW,
            Some(desktop.as_ptr() as *const c_void),
            None,
        );
        SHChangeNotify(
            SHCNE_UPDATEDIR,
            SHCNF_PATHW,
            Some(public.as_ptr() as *const c_void),
            None,
        );
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}

fn apply_moves(moves: &[PlannedMove], dry_run: bool, log_paths: Option<&Paths>) -> Result<()> {
    validate_moves(moves)?;
    if dry_run {
        for mv in moves {
            println!(
                "dry-run move: {} -> {}",
                path_display(&mv.from),
                path_display(&mv.to)
            );
        }
        return Ok(());
    }
    let mut completed: Vec<PlannedMove> = Vec::new();
    for mv in moves {
        if let Err(err) = move_path(&mv.from, &mv.to) {
            for done in completed.iter().rev() {
                if done.to.exists() && !done.from.exists() {
                    let from_w = wide(done.to.as_os_str());
                    let to_w = wide(done.from.as_os_str());
                    unsafe {
                        if MoveFileExW(
                            PCWSTR(from_w.as_ptr()),
                            PCWSTR(to_w.as_ptr()),
                            MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH,
                        )
                        .is_err()
                        {
                            if let Some(p) = log_paths {
                                log_line(
                                    p,
                                    &format!(
                                        "rollback move failed: {} -> {}",
                                        path_display(&done.to),
                                        path_display(&done.from)
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            return Err(err);
        }
        completed.push(mv.clone());
    }
    Ok(())
}

fn moves_from_to(
    from_dir: &Path,
    to_dir: &Path,
    config: &Config,
    log_paths: Option<&Paths>,
) -> Vec<PlannedMove> {
    child_entries(from_dir, log_paths)
        .into_iter()
        .filter(|item| should_manage_entry(item, config))
        .map(|item| PlannedMove {
            to: to_dir.join(item.file_name().unwrap_or_default()),
            from: item,
        })
        .collect()
}

fn set_active_desktop(p: &Paths, guid: &str) -> Result<()> {
    write_text_file(&p.active_file, guid)
}

fn adopt_current_desktop(p: &Paths, current_guid: &str) -> Result<()> {
    ensure_dirs(p)?;
    fs::create_dir_all(p.set_files(current_guid))?;
    save_layout(p, current_guid)?;
    set_active_desktop(p, current_guid)?;
    log_line(p, &format!("adopted current desktop {current_guid}"));
    Ok(())
}

fn switch_to_current_desktop(p: &Paths, dry_run: bool) -> Result<()> {
    let config = load_config(p);
    if !config.enabled {
        println!("DeskIcons is disabled");
        return Ok(());
    }
    if !dry_run {
        let _ = recover_journal(p, true)?;
    }
    let current = current_virtual_desktop_guid()
        .map(|g| guid_to_string(&g))
        .ok_or_else(|| AppError("Could not determine current virtual desktop GUID. Windows virtual desktop registry keys may have changed.".into()))?;
    ensure_dirs(p)?;
    fs::create_dir_all(p.set_files(&current))?;
    let Some(active) = read_text_file_trimmed(&p.active_file) else {
        println!("No active DeskIcons state exists; adopting current desktop {current}");
        if !dry_run {
            adopt_current_desktop(p, &current)?;
        }
        return Ok(());
    };
    if active == current {
        if !dry_run {
            save_layout(p, &current)?;
        }
        println!("Already active on {current}");
        return Ok(());
    }
    let active_files = p.set_files(&active);
    let target_files = p.set_files(&current);
    fs::create_dir_all(&active_files)?;
    fs::create_dir_all(&target_files)?;
    println!("Switching visible desktop icons");
    println!("  from: {active}");
    println!("  to:   {current}");
    log_line(p, &format!("switch {active} -> {current}"));
    if !dry_run {
        save_layout(p, &active)?;
    }
    let outbound = moves_from_to(&p.desktop, &active_files, &config, Some(p));
    let inbound = moves_from_to(&target_files, &p.desktop, &config, Some(p));
    let journal = Journal {
        stage: "planned".into(),
        from_guid: active.clone(),
        to_guid: current.clone(),
        outbound: outbound.clone(),
        inbound: inbound.clone(),
    };
    validate_moves(&outbound)?;
    if !dry_run {
        write_journal(p, journal, "planned")?;
    }
    apply_moves(&outbound, dry_run, Some(p))?;
    let journal = Journal {
        stage: "outbound-complete".into(),
        from_guid: active.clone(),
        to_guid: current.clone(),
        outbound: outbound.clone(),
        inbound: inbound.clone(),
    };
    if let Err(err) = (|| -> Result<()> {
        validate_moves(&inbound)?;
        if !dry_run {
            write_journal(p, journal, "outbound-complete")?;
        }
        apply_moves(&inbound, dry_run, Some(p))
    })() {
        if !dry_run {
            let rollback_journal = Journal {
                stage: "rollback".into(),
                from_guid: active.clone(),
                to_guid: current.clone(),
                outbound: outbound.clone(),
                inbound: inbound.clone(),
            };
            let _ = write_journal(p, rollback_journal, "rollback");
            let rollback: Vec<_> = outbound
                .iter()
                .map(|mv| PlannedMove {
                    from: mv.to.clone(),
                    to: mv.from.clone(),
                })
                .collect();
            if let Err(rollback_err) = apply_moves(&rollback, false, Some(p)) {
                log_error(p, &rollback_err);
                eprintln!(
                    "Rollback failed; manual recovery may be required under {}",
                    path_display(&p.root)
                );
            } else {
                set_active_desktop(p, &active)?;
                clear_journal(p);
            }
        }
        return Err(err);
    }
    if !dry_run {
        let complete_journal = Journal {
            stage: "inbound-complete".into(),
            from_guid: active.clone(),
            to_guid: current.clone(),
            outbound,
            inbound,
        };
        write_journal(p, complete_journal, "inbound-complete")?;
        set_active_desktop(p, &current)?;
        refresh_desktop(p);
        thread::sleep(Duration::from_millis(250));
        restore_layout(p, &current, true)?;
        clear_journal(p);
        log_line(p, &format!("switch complete {active} -> {current}"));
    }
    Ok(())
}

fn startup_command_for(target_exe: &Path) -> String {
    format!("\"{}\" tray", path_display(target_exe))
}

fn exe_path() -> Result<PathBuf> {
    let mut buffer = vec![0u16; MAX_PATH as usize];
    loop {
        let size = unsafe { GetModuleFileNameW(None, &mut buffer) } as usize;
        if size == 0 {
            return Err(last_error("GetModuleFileNameW"));
        }
        if size < buffer.len() {
            buffer.truncate(size);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

fn exe_dir() -> Result<PathBuf> {
    Ok(exe_path()?
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf())
}

fn startup_command() -> Result<String> {
    Ok(startup_command_for(&exe_path()?))
}

fn startup_enabled() -> bool {
    let mut ty = Default::default();
    let mut buffer = vec![0u16; 4096];
    let mut size = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            RUN_VALUE_NAME,
            RRF_RT_REG_SZ,
            Some(&mut ty),
            Some(buffer.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS || ty != REG_SZ {
        return false;
    }
    os_string_from_wide_z(&buffer).to_string_lossy() == startup_command().unwrap_or_default()
}

fn create_run_key() -> Result<RegKey> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            None,
            REG_OPEN_CREATE_OPTIONS(0),
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(AppError(format!(
            "Could not open Run registry key: {status:?}"
        )));
    }
    Ok(RegKey(key))
}

fn set_startup_enabled_for_exe(target_exe: &Path, enabled: bool) -> Result<()> {
    let key = create_run_key()?;
    let status = if enabled {
        let command = startup_command_for(target_exe);
        let command_w = wide_str(&command);
        unsafe {
            RegSetValueExW(
                key.raw(),
                RUN_VALUE_NAME,
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    command_w.as_ptr() as *const u8,
                    command_w.len() * 2,
                )),
            )
        }
    } else {
        let status = unsafe { RegDeleteValueW(key.raw(), RUN_VALUE_NAME) };
        if status == ERROR_FILE_NOT_FOUND {
            ERROR_SUCCESS
        } else {
            status
        }
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(AppError(format!(
            "Could not update startup registration: {status:?}"
        )))
    }
}

fn set_startup_enabled(enabled: bool) -> Result<()> {
    set_startup_enabled_for_exe(&exe_path()?, enabled)
}

fn open_path(path: &Path) -> Result<()> {
    let path_w = wide(path.as_os_str());
    let rc = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path_w.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if rc.0 as isize <= 32 {
        Err(AppError(format!(
            "ShellExecuteW failed opening {}: code {}",
            path_display(path),
            rc.0 as isize
        )))
    } else {
        Ok(())
    }
}

fn print_status(p: &Paths) {
    let config = load_config(p);
    println!("DeskIcons status");
    println!("  version:        {APP_VERSION}");
    println!("  user desktop:   {}", path_display(&p.desktop));
    println!("  public desktop: {}", path_display(&p.public_desktop));
    println!(
        "  managed scope:  user Desktop only; Public Desktop icons remain visible but unmanaged"
    );
    println!("  state root:     {}", path_display(&p.root));
    println!(
        "  enabled:        {}",
        if app_enabled(p) { "yes" } else { "no" }
    );
    println!(
        "  startup:        {}",
        if startup_enabled() { "yes" } else { "no" }
    );
    println!(
        "  manage files:   {}",
        if config.manage_non_shortcuts {
            "all user Desktop entries"
        } else {
            "user Desktop shortcuts only"
        }
    );
    println!("  poll ms:        {}", config.poll_ms);
    println!(
        "  journal:        {}",
        if p.journal_file.exists() {
            "pending"
        } else {
            "none"
        }
    );
    if path_display(&p.desktop)
        .to_ascii_lowercase()
        .contains("onedrive")
    {
        println!("  warning:        Desktop path appears to be OneDrive-synced");
    }
    println!(
        "  note:           virtual desktop detection uses undocumented Windows Explorer registry state"
    );
    if let Some(current) = current_virtual_desktop_guid() {
        println!("  current VD:     {}", guid_to_string(&current));
    } else {
        println!("  current VD:     <unknown>");
    }
    if let Some(active) = read_text_file_trimmed(&p.active_file) {
        println!("  active set:     {active}");
    } else {
        println!("  active set:     <none>");
    }
    let ids = virtual_desktop_ids();
    println!("  known VDs:      {}", ids.len());
    for (i, id) in ids.iter().enumerate() {
        println!("    [{i}] {}", guid_to_string(id));
    }
}

fn run_agent(p: &Paths, dry_run: bool) -> Result<()> {
    ensure_dirs(p)?;
    log_line(p, &format!("agent start version {APP_VERSION}"));
    if !dry_run {
        let _ = recover_journal(p, true)?;
    }
    if read_text_file_trimmed(&p.active_file).is_none() {
        if let Some(current) = current_virtual_desktop_guid() {
            let guid = guid_to_string(&current);
            println!("Initial adoption of current desktop {guid}");
            if !dry_run {
                adopt_current_desktop(p, &guid)?;
            }
        }
    }
    let mut last: Option<String> = None;
    println!("DeskIcons agent running. Press Ctrl+C to stop.");
    loop {
        let config = load_config(p);
        if let Some(current) = current_virtual_desktop_guid() {
            let guid = guid_to_string(&current);
            if last.is_none() {
                last = Some(guid);
            } else if last.as_deref() != Some(&guid) && config.enabled {
                switch_to_current_desktop(p, dry_run)?;
                last = Some(guid);
            } else if last.as_deref() != Some(&guid) {
                last = Some(guid);
            }
        }
        thread::sleep(Duration::from_millis(config.poll_ms));
    }
}

fn normalized_install_dir(p: &Paths) -> Option<PathBuf> {
    normalized_path(&p.root)
}

fn running_from_install_dir(p: &Paths) -> bool {
    let current = exe_dir().ok().and_then(|d| normalized_path(&d));
    let install = normalized_install_dir(p);
    current
        .zip(install)
        .is_some_and(|(a, b)| path_equal_ci(&a, &b))
}

fn process_path(handle: HANDLE) -> Option<PathBuf> {
    let mut buffer = vec![0u16; MAX_PATH as usize];
    let mut size = buffer.len() as u32;
    unsafe {
        if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok()
        {
            buffer.truncate(size as usize);
            Some(PathBuf::from(OsString::from_wide(&buffer)))
        } else {
            None
        }
    }
}

fn for_installed_processes(p: &Paths, mut f: impl FnMut(HANDLE)) {
    let Some(install_dir) = normalized_install_dir(p) else {
        return;
    };
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return;
    };
    let Some(snapshot) = Handle::new(snapshot) else {
        return;
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    unsafe {
        if Process32FirstW(snapshot.raw(), &mut entry).is_err() {
            return;
        }
        loop {
            let exe = os_string_from_wide_z(&entry.szExeFile);
            if exe.to_string_lossy().eq_ignore_ascii_case("deskicons.exe") {
                let rights = PROCESS_ACCESS_RIGHTS(
                    (PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE).0 | SYNCHRONIZE.0,
                );
                if let Ok(proc) = OpenProcess(rights, false, entry.th32ProcessID) {
                    if let Some(proc_path) = process_path(proc) {
                        if normalized_path(proc_path.parent().unwrap_or_else(|| Path::new("")))
                            .is_some_and(|d| path_equal_ci(&d, &install_dir))
                        {
                            f(proc);
                        }
                    }
                    let _ = CloseHandle(proc);
                }
            }
            if Process32NextW(snapshot.raw(), &mut entry).is_err() {
                break;
            }
        }
    }
}

fn installed_instance_running(p: &Paths) -> bool {
    let mut found = false;
    for_installed_processes(p, |_| found = true);
    found
}

fn kill_installed_instance(p: &Paths) {
    if let Ok(hwnd) = unsafe { FindWindowW(TRAY_CLASS, PCWSTR::null()) } {
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        for _ in 0..50 {
            if !installed_instance_running(p) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    for_installed_processes(p, |proc| unsafe {
        let _ = TerminateProcess(proc, 0);
        let _ = WaitForSingleObject(proc, 5000);
    });
}

fn message_box(text: &str, title: &str, flags: MESSAGEBOX_STYLE) -> i32 {
    let text_w = wide_str(text);
    let title_w = wide_str(title);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            flags,
        )
        .0
    }
}

fn show_install_dialog(is_update: bool, start_with_windows: &mut bool) -> bool {
    let strings = s();
    let button_label = if is_update {
        strings.update_button
    } else {
        strings.install_button
    };
    let window_title = if is_update {
        strings.update_title
    } else {
        strings.install_title
    };
    let main_instruction = if is_update {
        strings.update_instruction
    } else {
        strings.install_instruction
    };
    let content = if is_update {
        strings.update_content
    } else {
        strings.install_content
    };
    let button_w = wide_str(button_label);
    let cancel_w = wide_str(strings.cancel_button);
    let title_w = wide_str(window_title);
    let instruction_w = wide_str(main_instruction);
    let content_w = wide_str(content);
    let verification_w = wide_str(strings.install_start_with_windows);
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: 100,
            pszButtonText: PCWSTR(button_w.as_ptr()),
        },
        TASKDIALOG_BUTTON {
            nButtonID: IDCANCEL.0,
            pszButtonText: PCWSTR(cancel_w.as_ptr()),
        },
    ];
    let mut verification_checked = BOOL(1);
    let mut selected_button = IDCANCEL.0;
    let mut config = TASKDIALOGCONFIG {
        cbSize: mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION,
        pszWindowTitle: PCWSTR(title_w.as_ptr()),
        pszMainInstruction: PCWSTR(instruction_w.as_ptr()),
        pszContent: PCWSTR(content_w.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: 100,
        ..Default::default()
    };
    if !is_update {
        config.pszVerificationText = PCWSTR(verification_w.as_ptr());
    }
    let verification_out = if is_update {
        None
    } else {
        Some(&mut verification_checked as *mut BOOL)
    };
    let hr =
        unsafe { TaskDialogIndirect(&config, Some(&mut selected_button), None, verification_out) };
    if hr.is_err() {
        let fallback = if is_update {
            strings.update_fallback_content
        } else {
            strings.install_fallback_content
        };
        *start_with_windows = false;
        return message_box(
            fallback,
            window_title,
            MB_ICONQUESTION | MB_OKCANCEL | MB_DEFBUTTON1,
        ) == IDOK.0;
    }
    *start_with_windows = !is_update && verification_checked.as_bool();
    selected_button == 100
}

fn create_start_menu_shortcut(shortcut_path: &Path, target_exe: &Path) {
    let Ok(_co) = CoApartment::init() else { return };
    unsafe {
        let Ok(link) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER)
        else {
            return;
        };
        let target_w = wide(target_exe.as_os_str());
        let work_w = wide(
            target_exe
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .as_os_str(),
        );
        let desc_w = wide_str("DeskIcons");
        let _ = link.SetPath(PCWSTR(target_w.as_ptr()));
        let _ = link.SetWorkingDirectory(PCWSTR(work_w.as_ptr()));
        let _ = link.SetDescription(PCWSTR(desc_w.as_ptr()));
        if let Ok(pf) = link.cast::<IPersistFile>() {
            let shortcut_w = wide(shortcut_path.as_os_str());
            let _ = pf.Save(PCWSTR(shortcut_w.as_ptr()), true);
        }
    }
}

fn remove_start_menu_shortcut(shortcut_path: &Path) {
    let _ = fs::remove_file(shortcut_path);
}

fn register_uninstall_key(p: &Paths, target_exe: &Path) {
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\DeskIcons"),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return;
    }
    let key = RegKey(key);
    let set_sz = |name: PCWSTR, value: String| {
        let data = wide_str(&value);
        unsafe {
            let _ = RegSetValueExW(
                key.raw(),
                name,
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    data.as_ptr() as *const u8,
                    data.len() * 2,
                )),
            );
        }
    };
    let set_dw = |name: PCWSTR, value: u32| unsafe {
        let _ = RegSetValueExW(
            key.raw(),
            name,
            Some(0),
            REG_DWORD,
            Some(std::slice::from_raw_parts(
                (&value as *const u32) as *const u8,
                4,
            )),
        );
    };
    set_sz(w!("DisplayName"), "DeskIcons".into());
    set_sz(w!("DisplayVersion"), APP_VERSION.into());
    set_sz(w!("Publisher"), "DeskIcons".into());
    set_sz(w!("InstallLocation"), path_display(&p.root));
    set_sz(
        w!("UninstallString"),
        format!("\"{}\" uninstall", path_display(target_exe)),
    );
    set_sz(w!("DisplayIcon"), path_display(target_exe));
    set_dw(w!("NoModify"), 1);
    set_dw(w!("NoRepair"), 1);
}

fn remove_uninstall_key() {
    unsafe {
        let _ = RegDeleteKeyW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\DeskIcons"),
        );
    }
}

fn parked_files_exist(p: &Paths) -> bool {
    for set_dir in child_entries(&p.sets, Some(p)) {
        let files_dir = set_dir.join("files");
        if child_entries(&files_dir, Some(p))
            .into_iter()
            .next()
            .is_some()
        {
            return true;
        }
    }
    false
}

fn launch_uninstall_bat() {
    let temp = env::temp_dir();
    let bat_path = temp.join(format!("deskicons_uninstall_{}.bat", unsafe {
        GetCurrentProcessId()
    }));
    if let Ok(mut bat) = File::create(&bat_path) {
        let _ = bat.write_all(
            br#"@echo off
taskkill /f /im deskicons.exe >nul 2>&1
timeout /t 2 /nobreak >nul
rmdir /s /q "%LOCALAPPDATA%\DeskIcons"
del /f /q "%APPDATA%\Microsoft\Windows\Start Menu\Programs\DeskIcons.lnk" >nul 2>&1
echo.
if exist "%LOCALAPPDATA%\DeskIcons" (
  echo Some files could not be deleted automatically.
  echo Please delete them manually from: %LOCALAPPDATA%\DeskIcons
) else (
  echo DeskIcons uninstalled successfully.
)
echo.
timeout /t 5
del "%~f0"
"#,
        );
    }
    let mut cmdline = wide_str(&format!("cmd.exe /c \"{}\"", path_display(&bat_path)));
    unsafe {
        let si = STARTUPINFOW {
            cb: mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        if CreateProcessW(
            None,
            Some(PWSTR(cmdline.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NEW_CONSOLE,
            None,
            None,
            &si,
            &mut pi,
        )
        .is_ok()
        {
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
        }
    }
}

fn install_and_restart_if_needed(p: &Paths) -> Result<bool> {
    if running_from_install_dir(p) {
        return Ok(false);
    }
    let is_update = installed_instance_running(p);
    let mut start_with_windows = true;
    if !show_install_dialog(is_update, &mut start_with_windows) {
        return Ok(true);
    }
    if is_update {
        kill_installed_instance(p);
    }
    fs::create_dir_all(&p.root)?;
    let target_exe = p.root.join("deskicons.exe");
    let old_exe = p.root.join("deskicons.exe.old");
    if target_exe.exists() {
        let _ = fs::rename(&target_exe, &old_exe);
    }
    let exe = exe_path()?;
    let exe_w = wide(exe.as_os_str());
    let target_w = wide(target_exe.as_os_str());
    let mut copy_ok = false;
    for _ in 0..10 {
        unsafe {
            if CopyFileW(PCWSTR(exe_w.as_ptr()), PCWSTR(target_w.as_ptr()), false).is_ok() {
                copy_ok = true;
                break;
            }
            Sleep(300);
        }
    }
    if copy_ok {
        let target_ads = format!("{}:Zone.Identifier", path_display(&target_exe));
        let target_ads_w = wide_str(&target_ads);
        unsafe {
            let _ = DeleteFileW(PCWSTR(target_ads_w.as_ptr()));
        }
    }
    let _ = fs::remove_file(&old_exe);
    if !copy_ok {
        message_box(
            &format!(
                "{}From: {}\nTo:   {}",
                s().install_copy_error_prefix,
                path_display(&exe),
                path_display(&target_exe)
            ),
            s().install_copy_error_title,
            MB_OK | MB_ICONERROR,
        );
        return Ok(true);
    }
    if !is_update && start_with_windows {
        if let Err(err) = set_startup_enabled_for_exe(&target_exe, true) {
            message_box(&err.to_string(), s().install_title, MB_OK | MB_ICONWARNING);
        }
    }
    register_uninstall_key(p, &target_exe);
    create_start_menu_shortcut(&p.start_menu_shortcut, &target_exe);
    write_text_file(&p.install_notice_file, "1")?;
    let mut cmdline = wide_str(&format!("\"{}\"", path_display(&target_exe)));
    unsafe {
        let si = STARTUPINFOW {
            cb: mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        let cwd_w = wide(p.root.as_os_str());
        if CreateProcessW(
            PCWSTR(target_w.as_ptr()),
            Some(PWSTR(cmdline.as_mut_ptr())),
            None,
            None,
            false,
            Default::default(),
            None,
            PCWSTR(cwd_w.as_ptr()),
            &si,
            &mut pi,
        )
        .is_err()
        {
            message_box(
                s().install_started_error,
                if is_update {
                    s().update_title
                } else {
                    s().install_title
                },
                MB_OK | MB_ICONERROR,
            );
        } else {
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
        }
    }
    Ok(true)
}

struct TrayApp {
    paths: Paths,
    hwnd: HWND,
    tray_added: bool,
    gdiplus_token: usize,
    tray_icon: Option<OwnedIcon>,
    last_guid: Option<String>,
    stop_event: Option<Handle>,
    watcher: Option<thread::JoinHandle<()>>,
}

impl TrayApp {
    fn new(paths: Paths) -> Self {
        Self {
            paths,
            hwnd: HWND::default(),
            tray_added: false,
            gdiplus_token: 0,
            tray_icon: None,
            last_guid: None,
            stop_event: None,
            watcher: None,
        }
    }

    fn run(mut self) -> Result<i32> {
        ensure_dirs(&self.paths)?;
        let _ = recover_journal(&self.paths, false)?;
        self.start_gdiplus();
        unsafe {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(Self::window_proc),
                hInstance: HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
                lpszClassName: TRAY_CLASS,
                ..Default::default()
            };
            let atom = RegisterClassW(&wc);
            if atom == 0 && GetLastError() != ERROR_ALREADY_EXISTS {
                return Err(last_error("RegisterClassW"));
            }
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                TRAY_CLASS,
                w!("DeskIcons"),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0)),
                Some((&mut self as *mut TrayApp).cast()),
            );
            let hwnd = hwnd.map_err(|_| last_error("CreateWindowExW"))?;
            self.hwnd = hwnd;
            self.add_tray_icon()?;
            self.first_run_adopt();
            self.initialize_vd_state();
            self.start_vd_watcher();
            log_line(&self.paths, &format!("tray start version {APP_VERSION}"));
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        self.stop_vd_watcher();
        self.remove_tray_icon();
        self.stop_gdiplus();
        log_line(&self.paths, "tray exit");
        Ok(0)
    }

    extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            let app = if message == WM_NCCREATE {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let app = (*cs).lpCreateParams as *mut TrayApp;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as isize);
                app
            } else {
                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayApp
            };
            if !app.is_null() {
                return (*app).handle_message(hwnd, message, wparam, lparam);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }

    unsafe fn handle_message(
        &mut self,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_DESKICONS_VD_CHANGED => {
                self.on_vd_changed();
                LRESULT(0)
            }
            WM_COMMAND => {
                self.handle_command((wparam.0 & 0xffff) as u16);
                LRESULT(0)
            }
            WM_DESKICONS_TRAY => match (lparam.0 & 0xffff) as u32 {
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    self.show_menu();
                    LRESULT(0)
                }
                WM_LBUTTONDBLCLK => {
                    let _ = open_path(&self.paths.root);
                    LRESULT(0)
                }
                _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
            },
            WM_SETTINGCHANGE | WM_THEMECHANGED => LRESULT(0),
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn start_gdiplus(&mut self) {
        unsafe {
            let input = GdiplusStartupInput {
                GdiplusVersion: 1,
                ..Default::default()
            };
            let mut token = 0usize;
            if GdiplusStartup(&mut token, &input, null_mut()) == GDIP_OK {
                self.gdiplus_token = token;
            } else {
                log_line(&self.paths, "GDI+ startup failed; using fallback tray icon");
            }
        }
    }

    fn stop_gdiplus(&mut self) {
        self.tray_icon = None;
        if self.gdiplus_token != 0 {
            unsafe { GdiplusShutdown(self.gdiplus_token) };
            self.gdiplus_token = 0;
        }
    }

    fn load_png_file_icon(&self, path: &Path) -> Option<OwnedIcon> {
        if self.gdiplus_token == 0 || !path.exists() {
            return None;
        }
        unsafe {
            let path_w = wide(path.as_os_str());
            let mut bitmap: *mut GpBitmap = null_mut();
            if GdipCreateBitmapFromFile(PCWSTR(path_w.as_ptr()), &mut bitmap) != GDIP_OK
                || bitmap.is_null()
            {
                return None;
            }
            let mut icon = HICON::default();
            let ok = GdipCreateHICONFromBitmap(bitmap, &mut icon) == GDIP_OK && !icon.is_invalid();
            let _ = GdipDisposeImage(bitmap as *mut GpImage);
            if ok { Some(OwnedIcon(icon)) } else { None }
        }
    }

    fn load_tray_icon(&mut self) -> HICON {
        if let Ok(path) = exe_dir().map(|d| d.join("icon.png")) {
            if let Some(icon) = self.load_png_file_icon(&path) {
                let raw = icon.0;
                self.tray_icon = Some(icon);
                return raw;
            }
        }
        unsafe {
            let module = GetModuleHandleW(None).unwrap_or_default();
            LoadIconW(
                Some(HINSTANCE(module.0)),
                PCWSTR(IDI_APP_ICON as *const u16),
            )
            .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap_or_default())
        }
    }

    fn add_tray_icon(&mut self) -> Result<()> {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_DESKICONS_TRAY,
                hIcon: self.load_tray_icon(),
                ..Default::default()
            };
            let tip = wide_str("DeskIcons");
            copy_wide_truncated(&mut nid.szTip, &tip);
            check_bool(
                Shell_NotifyIconW(NIM_ADD, &nid),
                "Shell_NotifyIconW(NIM_ADD)",
            )?;
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            if !Shell_NotifyIconW(NIM_SETVERSION, &nid).as_bool() {
                log_line(&self.paths, "Shell_NotifyIconW(NIM_SETVERSION) failed");
            }
            self.tray_added = true;
        }
        Ok(())
    }

    fn remove_tray_icon(&mut self) {
        if self.hwnd.is_invalid() || !self.tray_added {
            return;
        }
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
        self.tray_added = false;
    }

    fn show_notification(&self, title: &str, text: &str) {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_INFO,
                dwInfoFlags: NIIF_INFO,
                ..Default::default()
            };
            let title_w = wide_str(title);
            let text_w = wide_str(text);
            copy_wide_truncated(&mut nid.szInfoTitle, &title_w);
            copy_wide_truncated(&mut nid.szInfo, &text_w);
            if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                log_line(
                    &self.paths,
                    "Shell_NotifyIconW(NIM_MODIFY notification) failed",
                );
            }
        }
    }

    fn first_run_adopt(&self) {
        let show_install_notice = self.paths.install_notice_file.exists();
        if read_text_file_trimmed(&self.paths.active_file).is_some() {
            if show_install_notice {
                self.show_notification(s().notif_active_title, s().notif_installed);
                let _ = fs::remove_file(&self.paths.install_notice_file);
            }
            return;
        }
        let Some(current) = current_virtual_desktop_guid() else {
            log_line(
                &self.paths,
                "could not adopt first run because current virtual desktop GUID was unavailable",
            );
            return;
        };
        if adopt_current_desktop(&self.paths, &guid_to_string(&current)).is_ok() {
            self.show_notification(s().notif_active_title, s().notif_adopted);
        }
        if show_install_notice {
            let _ = fs::remove_file(&self.paths.install_notice_file);
        }
    }

    fn show_menu(&self) {
        unsafe {
            let Ok(menu) = CreatePopupMenu() else {
                log_line(&self.paths, "CreatePopupMenu failed");
                return;
            };
            let _owned = OwnedMenu(menu);
            let enabled = app_enabled(&self.paths);
            let enabled_w = wide_str(s().menu_enabled);
            let adopt_w = wide_str(s().menu_adopt);
            let restore_w = wide_str(s().menu_restore_layout);
            let recover_w = wide_str(s().menu_recover);
            let open_w = wide_str(s().menu_open_state);
            let startup_w = wide_str(s().menu_startup);
            let language_w = wide_str(s().menu_language);
            let exit_w = wide_str(s().menu_exit);
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if enabled {
                        MF_CHECKED
                    } else {
                        Default::default()
                    },
                ID_TRAY_ENABLE as usize,
                PCWSTR(enabled_w.as_ptr()),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_TRAY_ADOPT as usize,
                PCWSTR(adopt_w.as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_TRAY_RESTORE as usize,
                PCWSTR(restore_w.as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_TRAY_RECOVER as usize,
                PCWSTR(recover_w.as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_TRAY_OPEN_STATE as usize,
                PCWSTR(open_w.as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if startup_enabled() {
                        MF_CHECKED
                    } else {
                        Default::default()
                    },
                ID_TRAY_STARTUP as usize,
                PCWSTR(startup_w.as_ptr()),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            if let Ok(lang_menu) = CreatePopupMenu() {
                for (idx, (language, label)) in
                    [(Language::En, "English"), (Language::De, "Deutsch")]
                        .iter()
                        .enumerate()
                {
                    let label_w = wide_str(label);
                    let _ = AppendMenuW(
                        lang_menu,
                        MF_STRING
                            | if *language == lang() {
                                MF_CHECKED
                            } else {
                                Default::default()
                            },
                        (ID_TRAY_LANG_BASE + idx as u16) as usize,
                        PCWSTR(label_w.as_ptr()),
                    );
                }
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP | MF_STRING,
                    lang_menu.0 as usize,
                    PCWSTR(language_w.as_ptr()),
                );
            }
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                ID_TRAY_EXIT as usize,
                PCWSTR(exit_w.as_ptr()),
            );
            let mut pt = POINT::default();
            if GetCursorPos(&mut pt).is_err() {
                log_line(&self.paths, "GetCursorPos failed");
                return;
            }
            let _ = SetForegroundWindow(self.hwnd);
            if !TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
                pt.x,
                pt.y,
                Some(0),
                self.hwnd,
                None,
            )
            .as_bool()
            {
                log_line(&self.paths, "TrackPopupMenu failed");
            }
        }
    }

    fn handle_command(&mut self, id: u16) {
        let result = match id {
            ID_TRAY_ENABLE => set_enabled(&self.paths, !app_enabled(&self.paths)),
            ID_TRAY_ADOPT => {
                if let Some(current) = current_virtual_desktop_guid() {
                    adopt_current_desktop(&self.paths, &guid_to_string(&current)).map(|_| {
                        message_box(s().msg_adopt_ok, s().msg_title, MB_OK | MB_ICONINFORMATION);
                    })
                } else {
                    message_box(s().msg_adopt_error, s().msg_title, MB_OK | MB_ICONERROR);
                    Ok(())
                }
            }
            ID_TRAY_RESTORE => current_virtual_desktop_guid()
                .map(|g| restore_layout(&self.paths, &guid_to_string(&g), false))
                .unwrap_or(Ok(())),
            ID_TRAY_RECOVER => recover_journal(&self.paths, true).map(|recovered| {
                if !recovered {
                    message_box(
                        s().msg_recover_nothing,
                        s().msg_title,
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }),
            ID_TRAY_OPEN_STATE => open_path(&self.paths.root),
            ID_TRAY_STARTUP => {
                let target = !startup_enabled();
                set_startup_enabled(target).map(|_| {
                    log_line(
                        &self.paths,
                        if target {
                            "enabled startup"
                        } else {
                            "disabled startup"
                        },
                    )
                })
            }
            ID_TRAY_EXIT => {
                unsafe {
                    let _ = DestroyWindow(self.hwnd);
                }
                Ok(())
            }
            id if (ID_TRAY_LANG_BASE..ID_TRAY_LANG_BASE + 2).contains(&id) => {
                let selected = if id == ID_TRAY_LANG_BASE {
                    Language::En
                } else {
                    Language::De
                };
                let mut config = load_config(&self.paths);
                config.language = Some(selected);
                save_config(&self.paths, &config).map(|_| {
                    set_language(selected);
                    log_line(
                        &self.paths,
                        &format!("language changed to {}", language_code(selected)),
                    );
                })
            }
            _ => Ok(()),
        };
        if let Err(err) = result {
            log_error(&self.paths, &err);
            message_box(&err.to_string(), s().msg_error_title, MB_OK | MB_ICONERROR);
        }
    }

    fn on_vd_changed(&mut self) {
        if !app_enabled(&self.paths) {
            return;
        }
        let Some(current) = current_virtual_desktop_guid().map(|g| guid_to_string(&g)) else {
            return;
        };
        if self.last_guid.is_none() {
            self.last_guid = Some(current);
            if let Err(err) = switch_to_current_desktop(&self.paths, false) {
                log_error(&self.paths, &err);
            }
        } else if self.last_guid.as_deref() != Some(&current) {
            if let Err(err) = switch_to_current_desktop(&self.paths, false) {
                log_error(&self.paths, &err);
            }
            self.last_guid = Some(current);
        }
    }

    fn initialize_vd_state(&mut self) {
        if !app_enabled(&self.paths) {
            return;
        }
        if let Some(current) = current_virtual_desktop_guid().map(|g| guid_to_string(&g)) {
            self.last_guid = Some(current);
            if let Err(err) = switch_to_current_desktop(&self.paths, false) {
                log_error(&self.paths, &err);
            }
        }
    }

    fn start_vd_watcher(&mut self) {
        let Ok(stop_raw) = (unsafe { CreateEventW(None, true, false, None) }) else {
            return;
        };
        let Some(stop_event) = Handle::new(stop_raw) else {
            return;
        };
        let stop_for_thread = stop_event.raw().0 as isize;
        let hwnd = self.hwnd.0 as isize;
        self.stop_event = Some(stop_event);
        self.watcher = Some(thread::spawn(move || {
            vd_watcher_thread(
                HWND(hwnd as *mut c_void),
                HANDLE(stop_for_thread as *mut c_void),
            )
        }));
    }

    fn stop_vd_watcher(&mut self) {
        if let Some(stop) = &self.stop_event {
            unsafe {
                let _ = SetEvent(stop.raw());
            }
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
        self.stop_event = None;
    }
}

fn vd_watcher_thread(hwnd: HWND, stop_event: HANDLE) {
    let mut session_id = 0;
    unsafe {
        let _ = ProcessIdToSessionId(GetCurrentProcessId(), &mut session_id);
    }
    let session_key = format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\SessionInfo\\{session_id}\\VirtualDesktops"
    );
    let Ok(main_event_raw) = (unsafe { CreateEventW(None, false, false, None) }) else {
        return;
    };
    let Some(main_event) = Handle::new(main_event_raw) else {
        return;
    };
    let Ok(sess_event_raw) = (unsafe { CreateEventW(None, false, false, None) }) else {
        return;
    };
    let Some(sess_event) = Handle::new(sess_event_raw) else {
        return;
    };
    loop {
        let main_key = open_notify_key(
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VirtualDesktops"),
            main_event.raw(),
        );
        let session_key_w = wide_str(&session_key);
        let sess_key = open_notify_key(PCWSTR(session_key_w.as_ptr()), sess_event.raw());
        let main_ok = main_key.is_some();
        let sess_ok = sess_key.is_some();
        if !main_ok && !sess_ok {
            if unsafe { WaitForSingleObject(stop_event, 2000) } == WAIT_OBJECT_0 {
                break;
            }
            continue;
        }
        let handles = [
            stop_event,
            main_event.raw(),
            if sess_ok {
                sess_event.raw()
            } else {
                stop_event
            },
        ];
        let count = if sess_ok { 3 } else { 2 };
        let wait = unsafe { WaitForMultipleObjects(&handles[..count], false, u32::MAX) };
        drop(main_key);
        drop(sess_key);
        if wait == WAIT_OBJECT_0 {
            break;
        }
        if wait.0 == WAIT_OBJECT_0.0 + 1 || wait.0 == WAIT_OBJECT_0.0 + 2 {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_DESKICONS_VD_CHANGED, WPARAM(0), LPARAM(0));
            }
        }
    }
}

fn open_notify_key(subkey: PCWSTR, event: HANDLE) -> Option<RegKey> {
    let mut raw = HKEY::default();
    let opened = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_NOTIFY, &mut raw) };
    if opened != ERROR_SUCCESS {
        return None;
    }
    let key = RegKey(raw);
    let notified = unsafe {
        RegNotifyChangeKeyValue(
            key.raw(),
            false,
            REG_NOTIFY_CHANGE_LAST_SET,
            Some(event),
            true,
        )
    };
    if notified == ERROR_SUCCESS {
        Some(key)
    } else {
        None
    }
}

fn export_state(p: &Paths) -> Result<PathBuf> {
    ensure_dirs(p)?;
    let stamp = timestamp_now().replace(':', "-").replace(' ', "_");
    let dest = p.exports.join(format!("deskicons-state-{stamp}"));
    fs::create_dir_all(&dest)?;
    fn copy_if_exists(from: &Path, to: &Path) {
        if !from.exists() {
            return;
        }
        if from.is_dir() {
            let _ = copy_dir_all(from, to);
        } else {
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(from, to);
        }
    }
    copy_if_exists(&p.config_file, &dest.join("config.ini"));
    copy_if_exists(&p.active_file, &dest.join("active-desktop.txt"));
    copy_if_exists(&p.journal_file, &dest.join("swap.journal"));
    copy_if_exists(&p.layouts, &dest.join("layouts"));
    copy_if_exists(&p.logs, &dest.join("logs"));
    let mut manifest = File::create(dest.join("manifest.txt"))?;
    writeln!(manifest, "DeskIcons {APP_VERSION}")?;
    writeln!(manifest, "exported={}", timestamp_now())?;
    writeln!(manifest, "user_desktop={}", path_display(&p.desktop))?;
    writeln!(
        manifest,
        "public_desktop={}",
        path_display(&p.public_desktop)
    )?;
    writeln!(manifest, "managed_scope=user_desktop_only")?;
    writeln!(
        manifest,
        "virtual_desktop_source=undocumented_explorer_registry_state"
    )?;
    if let Some(current) = current_virtual_desktop_guid() {
        writeln!(
            manifest,
            "current_virtual_desktop={}",
            guid_to_string(&current)
        )?;
    }
    writeln!(
        manifest,
        "startup_enabled={}",
        if startup_enabled() { 1 } else { 0 }
    )?;
    Ok(dest)
}

fn copy_dir_all(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn print_usage() {
    println!(
        "DeskIcons {APP_VERSION}\n\nUsage:\n  deskicons status\n  deskicons adopt --yes\n  deskicons switch-once [--dry-run]\n  deskicons restore-layout --yes\n  deskicons dump-visible\n  deskicons recover\n  deskicons enable|disable\n  deskicons startup on|off\n  deskicons export-state\n  deskicons tray\n  deskicons agent [--dry-run]\n\nNotes:\n  status is read-only.\n  adopt records the current virtual desktop as the owner of current user Desktop items.\n  switch-once swaps user Desktop folder contents if Windows is now on another virtual desktop.\n  restore-layout reapplies the saved icon positions for the current virtual desktop.\n  dump-visible prints Explorer's current user Desktop item names and positions.\n  recover completes or rolls back an interrupted journaled swap according to journal stage.\n  tray runs the tray UI.\n  agent polls the current virtual desktop and runs switch-once on desktop changes.\n  Public Desktop icons remain visible but unmanaged.\n  Virtual desktop detection depends on undocumented Windows Explorer registry state."
    );
}

fn attach_parent_console() {
    unsafe {
        let out = windows::Win32::System::Console::GetStdHandle(
            windows::Win32::System::Console::STD_OUTPUT_HANDLE,
        )
        .unwrap_or_default();
        if !out.is_invalid() && GetFileType(out) != FILE_TYPE_UNKNOWN {
            return;
        }
        let _ = AttachConsole(u32::MAX);
    }
}

fn acquire_single_instance() -> Result<Handle> {
    let mutex = unsafe { CreateMutexW(None, true, w!("Local\\DeskIcons.SingleInstance")) }
        .map_err(|_| last_error("CreateMutexW"))?;
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let Some(handle) = Handle::new(mutex) else {
        return Err(AppError(
            "Could not create DeskIcons single-instance mutex".into(),
        ));
    };
    if already_running {
        return Err(AppError(
            "Another DeskIcons instance is already running. Stop the tray instance before running this command.".into(),
        ));
    }
    Ok(handle)
}

fn command_needs_single_instance(command: &str) -> bool {
    matches!(
        command,
        "adopt"
            | "switch-once"
            | "restore-layout"
            | "recover"
            | "enable"
            | "disable"
            | "startup"
            | "uninstall"
            | "agent"
    )
}

fn command_main() -> Result<i32> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        let p = paths()?;
        set_language(
            load_config(&p)
                .language
                .unwrap_or_else(detect_system_language),
        );
        if install_and_restart_if_needed(&p)? {
            return Ok(0);
        }
        let _instance = acquire_single_instance()?;
        return TrayApp::new(p).run();
    }
    if args[0] != "tray" {
        attach_parent_console();
    }
    if matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_usage();
        return Ok(0);
    }
    let p = paths()?;
    set_language(
        load_config(&p)
            .language
            .unwrap_or_else(detect_system_language),
    );
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let _instance = if command_needs_single_instance(&args[0]) {
        Some(acquire_single_instance()?)
    } else {
        None
    };
    match args[0].as_str() {
        "status" => print_status(&p),
        "adopt" => {
            if !args.iter().any(|a| a == "--yes") {
                return Err(AppError("adopt requires --yes".into()));
            }
            let current = current_virtual_desktop_guid().ok_or_else(|| AppError("Could not determine current virtual desktop GUID. Windows virtual desktop registry keys may have changed.".into()))?;
            let guid = guid_to_string(&current);
            adopt_current_desktop(&p, &guid)?;
            println!("Adopted current desktop set {guid}");
        }
        "switch-once" => switch_to_current_desktop(&p, dry_run)?,
        "restore-layout" => {
            if !args.iter().any(|a| a == "--yes") {
                return Err(AppError("restore-layout requires --yes".into()));
            }
            let current = current_virtual_desktop_guid().ok_or_else(|| AppError("Could not determine current virtual desktop GUID. Windows virtual desktop registry keys may have changed.".into()))?;
            restore_layout(&p, &guid_to_string(&current), true)?;
        }
        "dump-visible" => dump_visible_items(&p)?,
        "recover" => {
            if !recover_journal(&p, true)? {
                println!("No interrupted swap journal exists.");
            }
        }
        "enable" => {
            set_enabled(&p, true)?;
            println!("DeskIcons enabled.");
        }
        "disable" => {
            set_enabled(&p, false)?;
            println!("DeskIcons disabled.");
        }
        "startup" => {
            if args.get(1).is_none_or(|v| v != "on" && v != "off") {
                return Err(AppError("startup requires 'on' or 'off'".into()));
            }
            set_startup_enabled(args[1] == "on")?;
            println!(
                "Startup {}.",
                if startup_enabled() {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        "export-state" => {
            let dest = export_state(&p)?;
            println!("Exported state to {}", path_display(&dest));
        }
        "uninstall" => {
            if parked_files_exist(&p) {
                return Err(AppError(format!(
                    "Refusing to uninstall because parked desktop files still exist under {}. Switch through your virtual desktops or export state before uninstalling so user data is not deleted.",
                    path_display(&p.sets)
                )));
            }
            let _ = set_startup_enabled_for_exe(&p.root.join("deskicons.exe"), false);
            remove_uninstall_key();
            remove_start_menu_shortcut(&p.start_menu_shortcut);
            launch_uninstall_bat();
        }
        "tray" => {
            if install_and_restart_if_needed(&p)? {
                return Ok(0);
            }
            let _instance = acquire_single_instance()?;
            return TrayApp::new(p).run();
        }
        "agent" => run_agent(&p, dry_run)?,
        _ => {
            print_usage();
            return Ok(2);
        }
    }
    Ok(0)
}

fn main() {
    match command_main() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_round_trips_unicode_names() {
        let value = r#"Desktop\ä-東京-😀 %.txt"#;
        let encoded = percent_encode(value);
        assert_ne!(encoded, value);
        assert_eq!(percent_decode(&encoded), value);
        assert!(encoded.contains("%C3%A4"));
        assert!(encoded.contains("%F0%9F%98%80"));
    }

    #[test]
    fn recovery_errors_when_both_move_paths_are_missing() {
        let base = env::temp_dir().join(format!("deskicons-test-{}", unsafe {
            GetCurrentProcessId()
        }));
        let _ = fs::remove_dir_all(&base);
        let mv = PlannedMove {
            from: base.join("missing-source"),
            to: base.join("missing-destination"),
        };
        let err = move_completed_or_finish(&mv).unwrap_err();
        assert!(
            err.to_string()
                .contains("Recovery lost both source and destination")
        );
    }

    #[test]
    fn fixed_wide_buffers_are_nul_terminated_when_truncated() {
        let source = wide_str("abcdef");
        let mut dest = [99u16; 4];
        copy_wide_truncated(&mut dest, &source);
        assert_eq!(&dest, &[b'a' as u16, b'b' as u16, b'c' as u16, 0]);
    }
}
