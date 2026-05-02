#include <windows.h>
#include <tlhelp32.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <exdisp.h>
#include <shlguid.h>
#include <shlwapi.h>
#include <shellapi.h>
#include <gdiplus.h>
#include <objidl.h>
#include <commctrl.h>

#include <algorithm>
#include <chrono>
#include <cctype>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <cwctype>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <map>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <thread>
#include <vector>

namespace fs = std::filesystem;

namespace {

constexpr const char *kAppVersion = "0.1.7";
constexpr UINT WM_DESKICONS_TRAY = WM_APP + 1;
constexpr UINT WM_DESKICONS_VD_CHANGED = WM_APP + 2;
constexpr wchar_t kTrayWindowClass[] = L"DeskIconsTrayWindow";
constexpr wchar_t kRunValueName[] = L"DeskIcons";
constexpr int IDR_TRAY_PNG = 101;

// ---------------------------------------------------------------------------
// Localization
// ---------------------------------------------------------------------------

enum class Language : int { en = 0, de = 1 };

struct Strings {
  // Install dialog
  const wchar_t *install_title;
  const wchar_t *install_instruction;
  const wchar_t *install_content;
  const wchar_t *install_button;
  const wchar_t *install_start_with_windows;
  const wchar_t *install_fallback_content;
  const wchar_t *install_copy_error_prefix;
  const wchar_t *install_copy_error_title;
  const wchar_t *install_started_error;
  // Update dialog
  const wchar_t *update_title;
  const wchar_t *update_instruction;
  const wchar_t *update_content;
  const wchar_t *update_button;
  const wchar_t *update_fallback_content;
  // Shared
  const wchar_t *cancel_button;
  // Tray menu
  const wchar_t *menu_enabled;
  const wchar_t *menu_restore_layout;
  const wchar_t *menu_recover;
  const wchar_t *menu_open_state;
  const wchar_t *menu_startup;
  const wchar_t *menu_language;
  const wchar_t *menu_exit;
  // MessageBoxes
  const wchar_t *msg_adopt_ok;
  const wchar_t *msg_adopt_error;
  const wchar_t *msg_recover_nothing;
  const wchar_t *msg_title;
  const wchar_t *msg_error_title;
  // Notifications
  const wchar_t *notif_active_title;
  const wchar_t *notif_installed;
  const wchar_t *notif_adopted;
};

constexpr Strings kStrings_en = {
    L"Install DeskIcons",
    L"Install DeskIcons for this user?",
    L"DeskIcons needs to be installed under your local application data folder "
    L"before it runs in the tray.",
    L"Install",
    L"Start automatically with Windows",
    L"Install DeskIcons under your local application data folder?",
    L"DeskIcons could not be installed:\n\n",
    L"Install DeskIcons",
    L"DeskIcons was installed, but the installed copy could not be started.",
    L"Update DeskIcons",
    L"Update DeskIcons?",
    L"Click Update to install the new version.",
    L"Update",
    L"Update DeskIcons in your local application data folder?",
    L"Cancel",
    L"Enabled",
    L"Restore Layout",
    L"Recover Interrupted Swap",
    L"Open State Folder",
    L"Start with Windows",
    L"Language",
    L"Exit",
    L"Current user Desktop adopted. Public Desktop icons remain unmanaged.",
    L"Could not determine the current virtual desktop.",
    L"No interrupted swap journal exists.",
    L"DeskIcons",
    L"DeskIcons Error",
    L"DeskIcons is now active",
    L"DeskIcons was installed and is running in the tray.",
    L"The current user Desktop was adopted for this virtual desktop. Public "
    L"Desktop icons are unmanaged.",
};

constexpr Strings kStrings_de = {
    L"DeskIcons installieren",
    L"DeskIcons f\u00fcr diesen Benutzer installieren?",
    L"DeskIcons muss in Ihrem lokalen App-Daten-Ordner installiert werden, "
    L"bevor es im Infobereich ausgef\u00fchrt werden kann.",
    L"Installieren",
    L"Automatisch mit Windows starten",
    L"DeskIcons im lokalen App-Daten-Ordner installieren?",
    L"DeskIcons konnte nicht installiert werden:\n\n",
    L"DeskIcons installieren",
    L"DeskIcons wurde installiert, aber die installierte Version konnte nicht "
    L"gestartet werden.",
    L"DeskIcons aktualisieren",
    L"DeskIcons aktualisieren?",
    L"Klicken Sie auf 'Aktualisieren', um die neue Version zu installieren.",
    L"Aktualisieren",
    L"DeskIcons im lokalen App-Daten-Ordner aktualisieren?",
    L"Abbrechen",
    L"Aktiviert",
    L"Layout wiederherstellen",
    L"Unterbrochenen Tausch fortsetzen",
    L"Statusordner \u00f6ffnen",
    L"Mit Windows starten",
    L"Sprache",
    L"Beenden",
    L"Der Desktop des aktuellen Benutzers wurde \u00fcbernommen. "
    L"\u00d6ffentliche Desktop-Icons bleiben nicht verwaltet.",
    L"Der aktuelle virtuelle Desktop konnte nicht ermittelt werden.",
    L"Kein unterbrochenes Tausch-Journal vorhanden.",
    L"DeskIcons",
    L"DeskIcons Fehler",
    L"DeskIcons ist jetzt aktiv",
    L"DeskIcons wurde installiert und l\u00e4uft im Infobereich.",
    L"Der aktuelle Desktop wurde f\u00fcr diesen virtuellen Desktop "
    L"\u00fcbernommen. \u00d6ffentliche Desktop-Icons sind nicht verwaltet.",
};

struct LangInfo {
  Language lang;
  const wchar_t *label;
  const char *code;
};

constexpr LangInfo kLanguages[] = {
    {Language::en, L"English", "en"},
    {Language::de, L"Deutsch", "de"},
};

Language g_language = Language::en;
const Strings *g_strings = &kStrings_en;

Language detect_system_language() {
  WORD primary = PRIMARYLANGID(GetUserDefaultUILanguage());
  if (primary == LANG_GERMAN)
    return Language::de;
  return Language::en;
}

Language language_from_code(std::string_view code) {
  for (const auto &info : kLanguages) {
    if (code == info.code)
      return info.lang;
  }
  return Language::en;
}

const char *language_code(Language lang) {
  for (const auto &info : kLanguages) {
    if (info.lang == lang)
      return info.code;
  }
  return "en";
}

void set_language(Language lang) {
  g_language = lang;
  switch (lang) {
  case Language::de:
    g_strings = &kStrings_de;
    break;
  default:
    g_strings = &kStrings_en;
    break;
  }
}

const Strings &S() { return *g_strings; }

// ---------------------------------------------------------------------------

struct AppError : std::runtime_error {
  using std::runtime_error::runtime_error;
};

struct CoInit {
  HRESULT hr = E_FAIL;
  bool initialized = false;

  CoInit() {
    hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    initialized = SUCCEEDED(hr);
  }

  ~CoInit() {
    if (initialized) {
      CoUninitialize();
    }
  }

  CoInit(const CoInit &) = delete;
  CoInit &operator=(const CoInit &) = delete;

  void require() const {
    if (FAILED(hr)) {
      throw AppError("COM initialization failed");
    }
  }
};

template <typename T> class ComPtr {
public:
  ComPtr() = default;
  explicit ComPtr(T *p) : ptr_(p) {}
  ~ComPtr() { reset(); }

  ComPtr(const ComPtr &) = delete;
  ComPtr &operator=(const ComPtr &) = delete;

  ComPtr(ComPtr &&other) noexcept : ptr_(other.ptr_) { other.ptr_ = nullptr; }

  ComPtr &operator=(ComPtr &&other) noexcept {
    if (this != &other) {
      reset();
      ptr_ = other.ptr_;
      other.ptr_ = nullptr;
    }
    return *this;
  }

  T *get() const { return ptr_; }

  T **put() {
    reset();
    return &ptr_;
  }

  T *detach() {
    T *p = ptr_;
    ptr_ = nullptr;
    return p;
  }

  T *operator->() const { return ptr_; }
  explicit operator bool() const { return ptr_ != nullptr; }

  void reset(T *p = nullptr) {
    if (ptr_) {
      ptr_->Release();
    }
    ptr_ = p;
  }

private:
  T *ptr_ = nullptr;
};

struct UniqueHKey {
  HKEY h = nullptr;
  ~UniqueHKey() { reset(); }

  UniqueHKey() = default;
  UniqueHKey(const UniqueHKey &) = delete;
  UniqueHKey &operator=(const UniqueHKey &) = delete;

  HKEY *put() {
    reset();
    return &h;
  }

  void reset(HKEY value = nullptr) {
    if (h) {
      RegCloseKey(h);
    }
    h = value;
  }

  operator HKEY() const { return h; }
};

struct UniqueHMenu {
  HMENU h = nullptr;
  explicit UniqueHMenu(HMENU value = nullptr) : h(value) {}
  ~UniqueHMenu() { reset(); }

  UniqueHMenu(const UniqueHMenu &) = delete;
  UniqueHMenu &operator=(const UniqueHMenu &) = delete;

  void reset(HMENU value = nullptr) {
    if (h) {
      DestroyMenu(h);
    }
    h = value;
  }

  operator HMENU() const { return h; }
};

struct UniqueHIcon {
  HICON h = nullptr;
  UniqueHIcon() = default;
  explicit UniqueHIcon(HICON value) : h(value) {}
  ~UniqueHIcon() { reset(); }

  UniqueHIcon(const UniqueHIcon &) = delete;
  UniqueHIcon &operator=(const UniqueHIcon &) = delete;

  UniqueHIcon(UniqueHIcon &&other) noexcept : h(other.h) { other.h = nullptr; }

  UniqueHIcon &operator=(UniqueHIcon &&other) noexcept {
    if (this != &other) {
      reset();
      h = other.h;
      other.h = nullptr;
    }
    return *this;
  }

  HICON get() const { return h; }

  HICON detach() {
    HICON value = h;
    h = nullptr;
    return value;
  }

  void reset(HICON value = nullptr) {
    if (h) {
      DestroyIcon(h);
    }
    h = value;
  }

  explicit operator bool() const { return h != nullptr; }
};

struct UniqueHGlobal {
  HGLOBAL h = nullptr;
  explicit UniqueHGlobal(HGLOBAL value = nullptr) : h(value) {}
  ~UniqueHGlobal() { reset(); }

  UniqueHGlobal(const UniqueHGlobal &) = delete;
  UniqueHGlobal &operator=(const UniqueHGlobal &) = delete;

  HGLOBAL get() const { return h; }

  HGLOBAL detach() {
    HGLOBAL value = h;
    h = nullptr;
    return value;
  }

  void reset(HGLOBAL value = nullptr) {
    if (h) {
      GlobalFree(h);
    }
    h = value;
  }
};

struct UniqueHandle {
  HANDLE h = nullptr;
  UniqueHandle() = default;
  explicit UniqueHandle(HANDLE value) : h(value) {}
  ~UniqueHandle() { reset(); }

  UniqueHandle(const UniqueHandle &) = delete;
  UniqueHandle &operator=(const UniqueHandle &) = delete;

  void reset(HANDLE value = nullptr) {
    if (h)
      CloseHandle(h);
    h = value;
  }

  explicit operator bool() const { return h != nullptr; }
};

std::wstring widen(std::string_view s) {
  if (s.empty()) {
    return {};
  }
  if (s.size() > static_cast<size_t>(std::numeric_limits<int>::max())) {
    throw AppError("UTF-8 string is too large to convert");
  }
  int needed = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s.data(),
                                   static_cast<int>(s.size()), nullptr, 0);
  if (needed <= 0) {
    throw AppError("UTF-8 to UTF-16 conversion failed");
  }
  std::wstring out(static_cast<size_t>(needed), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s.data(),
                          static_cast<int>(s.size()), out.data(),
                          needed) != needed) {
    throw AppError("UTF-8 to UTF-16 conversion failed");
  }
  return out;
}

std::string narrow(std::wstring_view s) {
  if (s.empty()) {
    return {};
  }
  if (s.size() > static_cast<size_t>(std::numeric_limits<int>::max())) {
    throw AppError("UTF-16 string is too large to convert");
  }
  int needed = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, s.data(),
                                   static_cast<int>(s.size()), nullptr, 0,
                                   nullptr, nullptr);
  if (needed <= 0) {
    throw AppError("UTF-16 to UTF-8 conversion failed");
  }
  std::string out(static_cast<size_t>(needed), '\0');
  if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, s.data(),
                          static_cast<int>(s.size()), out.data(), needed,
                          nullptr, nullptr) != needed) {
    throw AppError("UTF-16 to UTF-8 conversion failed");
  }
  return out;
}

std::wstring to_lower_w(std::wstring value) {
  std::transform(value.begin(), value.end(), value.begin(), [](wchar_t c) {
    return static_cast<wchar_t>(std::towlower(c));
  });
  return value;
}

std::string trim_ascii(std::string value) {
  auto is_space = [](unsigned char c) { return std::isspace(c) != 0; };
  auto first = std::find_if_not(value.begin(), value.end(), is_space);
  auto last = std::find_if_not(value.rbegin(), value.rend(), is_space).base();
  if (first >= last) {
    return {};
  }
  return std::string(first, last);
}

std::string win32_error(DWORD code) {
  LPWSTR buffer = nullptr;
  DWORD len = FormatMessageW(
      FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM |
          FORMAT_MESSAGE_IGNORE_INSERTS,
      nullptr, code, 0, reinterpret_cast<LPWSTR>(&buffer), 0, nullptr);
  std::wstring msg = len ? std::wstring(buffer, len) : L"unknown error";
  if (buffer) {
    LocalFree(buffer);
  }
  while (!msg.empty() &&
         (msg.back() == L'\r' || msg.back() == L'\n' || msg.back() == L' ')) {
    msg.pop_back();
  }
  return narrow(msg) + " (" + std::to_string(code) + ")";
}

std::string lstatus_error(LSTATUS status) {
  return win32_error(static_cast<DWORD>(status));
}

void check_win32(bool ok, const std::string &what) {
  if (!ok) {
    throw AppError(what + ": " + win32_error(GetLastError()));
  }
}

void check_lstatus(LSTATUS status, const std::string &what) {
  if (status != ERROR_SUCCESS) {
    throw AppError(what + ": " + lstatus_error(status));
  }
}

void check_hr(HRESULT hr, const std::string &what) {
  if (FAILED(hr)) {
    std::ostringstream oss;
    oss << what << ": HRESULT 0x" << std::hex << static_cast<unsigned long>(hr);
    throw AppError(oss.str());
  }
}

std::wstring known_folder_path(const KNOWNFOLDERID &id) {
  PWSTR raw = nullptr;
  HRESULT hr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, nullptr, &raw);
  check_hr(hr, "SHGetKnownFolderPath");
  std::wstring path(raw);
  CoTaskMemFree(raw);
  return path;
}

std::wstring env_path(const wchar_t *name) {
  SetLastError(ERROR_SUCCESS);
  DWORD needed = GetEnvironmentVariableW(name, nullptr, 0);
  if (needed == 0) {
    DWORD err = GetLastError();
    if (err == ERROR_ENVVAR_NOT_FOUND) {
      throw AppError("Required environment variable is missing: " +
                     narrow(name));
    }
    throw AppError("Could not query environment variable " + narrow(name) +
                   ": " + win32_error(err));
  }
  std::wstring out(needed, L'\0');
  DWORD written = GetEnvironmentVariableW(name, out.data(), needed);
  if (written == 0 || written >= needed) {
    throw AppError("Could not read environment variable " + narrow(name));
  }
  out.resize(written);
  return out;
}

std::string guid_to_string(const GUID &guid) {
  wchar_t buffer[64]{};
  if (StringFromGUID2(guid, buffer, static_cast<int>(std::size(buffer))) == 0) {
    throw AppError("StringFromGUID2 failed");
  }
  std::wstring w(buffer);
  if (!w.empty() && w.front() == L'{') {
    w.erase(w.begin());
  }
  if (!w.empty() && w.back() == L'}') {
    w.pop_back();
  }
  std::string s = narrow(w);
  std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) {
    return static_cast<char>(std::tolower(c));
  });
  return s;
}

std::optional<GUID> read_guid_value(HKEY root, const wchar_t *subkey,
                                    const wchar_t *value_name) {
  GUID guid{};
  DWORD type = 0;
  DWORD size = sizeof(guid);
  LSTATUS status = RegGetValueW(root, subkey, value_name, RRF_RT_REG_BINARY,
                                &type, &guid, &size);
  if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) {
    return std::nullopt;
  }
  if (status != ERROR_SUCCESS || type != REG_BINARY || size != sizeof(guid)) {
    return std::nullopt;
  }
  return guid;
}

std::optional<std::vector<GUID>>
read_guid_array_value(HKEY root, const wchar_t *subkey,
                      const wchar_t *value_name) {
  DWORD type = 0;
  DWORD size = 0;
  LSTATUS status = RegGetValueW(root, subkey, value_name, RRF_RT_REG_BINARY,
                                &type, nullptr, &size);
  if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) {
    return std::nullopt;
  }
  if (status != ERROR_SUCCESS || type != REG_BINARY || size == 0 ||
      (size % sizeof(GUID)) != 0) {
    return std::nullopt;
  }

  const DWORD expected_size = size;
  std::vector<GUID> ids(size / sizeof(GUID));
  status = RegGetValueW(root, subkey, value_name, RRF_RT_REG_BINARY, &type,
                        ids.data(), &size);
  if (status != ERROR_SUCCESS || type != REG_BINARY || size != expected_size) {
    return std::nullopt;
  }
  return ids;
}

std::optional<GUID> current_virtual_desktop_guid() {
  constexpr auto key = L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer"
                       L"\\VirtualDesktops";
  if (auto guid =
          read_guid_value(HKEY_CURRENT_USER, key, L"CurrentVirtualDesktop")) {
    return guid;
  }

  DWORD session_id = 0;
  if (ProcessIdToSessionId(GetCurrentProcessId(), &session_id)) {
    std::wstring fallback = L"Software\\Microsoft\\Windows\\CurrentVersion\\Exp"
                            L"lorer\\SessionInfo\\" +
                            std::to_wstring(session_id) + L"\\VirtualDesktops";
    if (auto guid = read_guid_value(HKEY_CURRENT_USER, fallback.c_str(),
                                    L"CurrentVirtualDesktop")) {
      return guid;
    }
  }

  return std::nullopt;
}

std::vector<GUID> virtual_desktop_ids() {
  constexpr auto key = L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer"
                       L"\\VirtualDesktops";
  if (auto ids =
          read_guid_array_value(HKEY_CURRENT_USER, key, L"VirtualDesktopIDs")) {
    return *ids;
  }
  return {};
}

struct Paths {
  fs::path desktop;
  fs::path public_desktop;
  fs::path root;
  fs::path sets;
  fs::path layouts;
  fs::path logs;
  fs::path exports;
  fs::path config_file;
  fs::path journal_file;
  fs::path active_file;
  fs::path disabled_file;
  fs::path install_notice_file;

  fs::path start_menu_shortcut;

  fs::path set_files(const std::string &guid) const {
    return sets / widen(guid) / L"files";
  }

  fs::path layout_file(const std::string &guid) const {
    return layouts / (widen(guid) + L".tsv");
  }
};

Paths paths() {
  CoInit co;
  co.require();
  fs::path root = fs::path(env_path(L"LOCALAPPDATA")) / L"DeskIcons";
  return Paths{
      fs::path(known_folder_path(FOLDERID_Desktop)),
      fs::path(known_folder_path(FOLDERID_PublicDesktop)),
      root,
      root / L"sets",
      root / L"layouts",
      root / L"logs",
      root / L"exports",
      root / L"config.ini",
      root / L"swap.journal",
      root / L"active-desktop.txt",
      root / L"disabled",
      root / L"install-notice",
      fs::path(known_folder_path(FOLDERID_Programs)) / L"DeskIcons.lnk",
  };
}

void ensure_dirs(const Paths &p) {
  fs::create_directories(p.sets);
  fs::create_directories(p.layouts);
  fs::create_directories(p.logs);
  fs::create_directories(p.exports);
}

std::string timestamp_now() {
  auto now = std::chrono::system_clock::now();
  std::time_t t = std::chrono::system_clock::to_time_t(now);
  std::tm tm{};
  localtime_s(&tm, &t);
  std::ostringstream out;
  out << std::put_time(&tm, "%Y-%m-%d %H:%M:%S");
  return out.str();
}

void log_line(const Paths &p, std::string_view message) {
  try {
    fs::create_directories(p.logs);
    std::ofstream out(p.logs / L"deskicons.log",
                      std::ios::binary | std::ios::app);
    out << timestamp_now() << " " << message << "\n";
  } catch (...) {
  }
}

void log_error(const Paths &p, const std::exception &ex) {
  log_line(p, std::string("ERROR ") + ex.what());
}

bool startup_enabled();

bool windows_system_uses_light_theme() {
  DWORD value = 1;
  DWORD type = 0;
  DWORD size = sizeof(value);
  LSTATUS status = RegGetValueW(
      HKEY_CURRENT_USER,
      L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
      L"SystemUsesLightTheme", RRF_RT_REG_DWORD, &type, &value, &size);
  if (status != ERROR_SUCCESS || type != REG_DWORD || size != sizeof(value)) {
    return true;
  }
  return value != 0;
}

struct Config {
  bool enabled = true;
  bool manage_non_shortcuts = true;
  DWORD poll_ms = 750;
  std::optional<Language> language; // nullopt = auto-detect from system locale
};

Config load_config(const Paths &p) {
  Config config;
  std::ifstream in(p.config_file, std::ios::binary);
  if (!in) {
    return config;
  }

  std::string line;
  while (std::getline(in, line)) {
    std::string trimmed = trim_ascii(line);
    if (trimmed.empty() || trimmed[0] == '#' || trimmed[0] == ';') {
      continue;
    }
    auto pos = trimmed.find('=');
    if (pos == std::string::npos) {
      continue;
    }
    std::string key = trim_ascii(trimmed.substr(0, pos));
    std::string value = trim_ascii(trimmed.substr(pos + 1));
    if (key == "enabled") {
      config.enabled = value != "0";
    } else if (key == "manage_non_shortcuts") {
      config.manage_non_shortcuts = value != "0";
    } else if (key == "poll_ms") {
      try {
        unsigned long parsed = std::stoul(value);
        parsed = std::clamp<unsigned long>(parsed, 250, 10000);
        config.poll_ms = static_cast<DWORD>(parsed);
      } catch (...) {
        config.poll_ms = 750;
      }
    } else if (key == "language") {
      config.language = language_from_code(value);
    }
  }
  return config;
}

void save_config(const Paths &p, const Config &config) {
  fs::create_directories(p.root);
  std::ofstream out(p.config_file, std::ios::binary | std::ios::trunc);
  if (!out) {
    throw AppError("Could not write config file");
  }
  out << "enabled=" << (config.enabled ? "1" : "0") << "\n";
  out << "manage_non_shortcuts=" << (config.manage_non_shortcuts ? "1" : "0")
      << "\n";
  out << "poll_ms=" << config.poll_ms << "\n";
  if (config.language.has_value()) {
    out << "language=" << language_code(*config.language) << "\n";
  }
}

bool app_enabled(const Paths &p) {
  Config config = load_config(p);
  return config.enabled && !fs::exists(p.disabled_file);
}

void set_enabled(const Paths &p, bool enabled) {
  Config config = load_config(p);
  config.enabled = enabled;
  save_config(p, config);
  if (enabled) {
    std::error_code ec;
    fs::remove(p.disabled_file, ec);
  } else {
    fs::create_directories(p.root);
    std::ofstream out(p.disabled_file, std::ios::binary | std::ios::trunc);
    if (!out) {
      throw AppError("Could not write disabled marker");
    }
    out << "disabled\n";
  }
  log_line(p, enabled ? "enabled agent" : "disabled agent");
}

std::optional<std::string> read_text_file_trimmed(const fs::path &path) {
  std::ifstream in(path, std::ios::binary);
  if (!in) {
    return std::nullopt;
  }
  std::string value((std::istreambuf_iterator<char>(in)),
                    std::istreambuf_iterator<char>());
  value = trim_ascii(value);
  return value.empty() ? std::nullopt : std::optional<std::string>(value);
}

void write_text_file(const fs::path &path, std::string_view value) {
  fs::create_directories(path.parent_path());
  std::ofstream out(path, std::ios::binary | std::ios::trunc);
  if (!out) {
    throw AppError("Could not write " + narrow(path.wstring()));
  }
  out.write(value.data(), static_cast<std::streamsize>(value.size()));
  out.write("\n", 1);
}

bool is_skipped_desktop_entry(const fs::path &path) {
  std::wstring name = to_lower_w(path.filename().wstring());
  return name == L"desktop.ini";
}

std::vector<fs::path> child_entries(const fs::path &dir,
                                    const Paths *log_paths = nullptr) {
  std::vector<fs::path> entries;
  std::error_code ec;
  if (!fs::exists(dir, ec)) {
    return entries;
  }

  fs::directory_iterator it(dir, fs::directory_options::skip_permission_denied,
                            ec);
  if (ec) {
    if (log_paths) {
      log_line(*log_paths, "could not enumerate " + narrow(dir.wstring()) +
                               ": " + ec.message());
    }
    return entries;
  }

  for (const auto &entry : it) {
    try {
      if (!is_skipped_desktop_entry(entry.path())) {
        entries.push_back(entry.path());
      }
    } catch (const std::exception &ex) {
      if (log_paths) {
        log_line(*log_paths,
                 std::string("skipped desktop entry during enumeration: ") +
                     ex.what());
      }
    }
  }
  std::sort(entries.begin(), entries.end());
  return entries;
}

bool should_manage_entry(const fs::path &path, const Config &config) {
  if (config.manage_non_shortcuts) {
    return true;
  }
  std::wstring ext = to_lower_w(path.extension().wstring());
  return ext == L".lnk";
}

std::string percent_encode(std::string_view value) {
  constexpr char hex[] = "0123456789ABCDEF";
  std::string out;
  for (unsigned char c : value) {
    if (c == '%' || c == '\t' || c == '\n' || c == '\r') {
      out.push_back('%');
      out.push_back(hex[c >> 4]);
      out.push_back(hex[c & 0x0F]);
    } else {
      out.push_back(static_cast<char>(c));
    }
  }
  return out;
}

int hex_value(char c) {
  if (c >= '0' && c <= '9')
    return c - '0';
  if (c >= 'a' && c <= 'f')
    return c - 'a' + 10;
  if (c >= 'A' && c <= 'F')
    return c - 'A' + 10;
  return -1;
}

std::string percent_decode(std::string_view value) {
  std::string out;
  for (size_t i = 0; i < value.size(); ++i) {
    if (value[i] == '%' && i + 2 < value.size()) {
      int hi = hex_value(value[i + 1]);
      int lo = hex_value(value[i + 2]);
      if (hi >= 0 && lo >= 0) {
        out.push_back(static_cast<char>((hi << 4) | lo));
        i += 2;
        continue;
      }
    }
    out.push_back(value[i]);
  }
  return out;
}

struct PlannedMove {
  fs::path from;
  fs::path to;
};

void move_path(const fs::path &from, const fs::path &to);
void set_active_desktop(const Paths &p, const std::string &guid);
void refresh_desktop(const Paths &p);
void restore_layout(const Paths &p, const std::string &guid, bool verbose);

struct Journal {
  std::string stage;
  std::string from_guid;
  std::string to_guid;
  std::vector<PlannedMove> outbound;
  std::vector<PlannedMove> inbound;
};

std::string encode_path(const fs::path &path) {
  return percent_encode(narrow(path.wstring()));
}

fs::path decode_path(std::string_view value) {
  return fs::path(widen(percent_decode(value)));
}

void write_journal(const Paths &p, Journal journal, std::string_view stage) {
  journal.stage = std::string(stage);
  fs::create_directories(p.root);
  std::ofstream out(p.journal_file, std::ios::binary | std::ios::trunc);
  if (!out) {
    throw AppError("Could not write swap journal");
  }
  out << "version\t1\n";
  out << "stage\t" << journal.stage << "\n";
  out << "from\t" << journal.from_guid << "\n";
  out << "to\t" << journal.to_guid << "\n";
  for (const auto &move : journal.outbound) {
    out << "out\t" << encode_path(move.from) << "\t" << encode_path(move.to)
        << "\n";
  }
  for (const auto &move : journal.inbound) {
    out << "in\t" << encode_path(move.from) << "\t" << encode_path(move.to)
        << "\n";
  }
}

std::optional<Journal> read_journal(const Paths &p) {
  std::ifstream in(p.journal_file, std::ios::binary);
  if (!in) {
    return std::nullopt;
  }

  Journal journal;
  std::string line;
  while (std::getline(in, line)) {
    std::vector<std::string> parts;
    std::string part;
    std::istringstream iss(line);
    while (std::getline(iss, part, '\t')) {
      parts.push_back(part);
    }
    if (parts.size() >= 2 && parts[0] == "stage") {
      journal.stage = parts[1];
    } else if (parts.size() >= 2 && parts[0] == "from") {
      journal.from_guid = parts[1];
    } else if (parts.size() >= 2 && parts[0] == "to") {
      journal.to_guid = parts[1];
    } else if (parts.size() >= 3 && parts[0] == "out") {
      journal.outbound.push_back(
          {decode_path(parts[1]), decode_path(parts[2])});
    } else if (parts.size() >= 3 && parts[0] == "in") {
      journal.inbound.push_back({decode_path(parts[1]), decode_path(parts[2])});
    }
  }

  if (journal.stage.empty()) {
    journal.stage = "planned";
  }
  if (journal.from_guid.empty() || journal.to_guid.empty()) {
    throw AppError("Swap journal is malformed");
  }
  return journal;
}

void clear_journal(const Paths &p) {
  std::error_code ec;
  fs::remove(p.journal_file, ec);
}

bool move_completed_or_finish(const PlannedMove &move) {
  bool src_exists = fs::exists(move.from);
  bool dst_exists = fs::exists(move.to);
  if (src_exists && dst_exists) {
    throw AppError("Recovery conflict: both source and destination exist for " +
                   narrow(move.from.wstring()));
  }
  if (src_exists && !dst_exists) {
    move_path(move.from, move.to);
    return true;
  }
  return dst_exists;
}

void finish_move_set(const std::vector<PlannedMove> &moves) {
  for (const auto &move : moves) {
    move_completed_or_finish(move);
  }
}

void rollback_move_set(const std::vector<PlannedMove> &moves) {
  for (auto it = moves.rbegin(); it != moves.rend(); ++it) {
    PlannedMove rollback{it->to, it->from};
    move_completed_or_finish(rollback);
  }
}

bool recover_journal(const Paths &p, bool verbose) {
  auto journal = read_journal(p);
  if (!journal) {
    return false;
  }

  if (verbose) {
    std::cout << "Recovering interrupted swap\n";
    std::cout << "  stage: " << journal->stage << "\n";
    std::cout << "  from:  " << journal->from_guid << "\n";
    std::cout << "  to:    " << journal->to_guid << "\n";
  }
  log_line(p, "recovering interrupted swap stage=" + journal->stage + " " +
                  journal->from_guid + " -> " + journal->to_guid);

  if (journal->stage == "planned") {
    rollback_move_set(journal->outbound);
    set_active_desktop(p, journal->from_guid);
  } else if (journal->stage == "outbound-complete" ||
             journal->stage == "inbound-complete") {
    finish_move_set(journal->outbound);
    finish_move_set(journal->inbound);
    set_active_desktop(p, journal->to_guid);
    refresh_desktop(p);
    restore_layout(p, journal->to_guid, verbose);
  } else if (journal->stage == "rollback") {
    rollback_move_set(journal->inbound);
    rollback_move_set(journal->outbound);
    set_active_desktop(p, journal->from_guid);
  } else {
    throw AppError("Swap journal has unknown stage: " + journal->stage);
  }

  clear_journal(p);
  refresh_desktop(p);
  log_line(p, "recovered interrupted swap");
  return true;
}

struct DesktopViewContext {
  ComPtr<IShellView> shell_view;
  ComPtr<IFolderView> folder_view;
};

DesktopViewContext desktop_view_context() {
  ComPtr<IShellWindows> shell_windows;
  check_hr(CoCreateInstance(CLSID_ShellWindows, nullptr, CLSCTX_ALL,
                            IID_PPV_ARGS(shell_windows.put())),
           "CoCreateInstance(CLSID_ShellWindows)");

  VARIANT vt_loc;
  VariantInit(&vt_loc);
  vt_loc.vt = VT_I4;
  vt_loc.lVal = CSIDL_DESKTOP;

  VARIANT vt_empty;
  VariantInit(&vt_empty);

  long hwnd = 0;
  ComPtr<IDispatch> dispatch;
  check_hr(shell_windows->FindWindowSW(&vt_loc, &vt_empty, SWC_DESKTOP, &hwnd,
                                       SWFO_NEEDDISPATCH, dispatch.put()),
           "IShellWindows::FindWindowSW(SWC_DESKTOP)");

  ComPtr<IServiceProvider> service_provider;
  check_hr(dispatch->QueryInterface(IID_PPV_ARGS(service_provider.put())),
           "IDispatch::QueryInterface(IServiceProvider)");

  ComPtr<IShellBrowser> browser;
  check_hr(service_provider->QueryService(SID_STopLevelBrowser,
                                          IID_PPV_ARGS(browser.put())),
           "IServiceProvider::QueryService(SID_STopLevelBrowser)");

  ComPtr<IShellView> shell_view;
  check_hr(browser->QueryActiveShellView(shell_view.put()),
           "IShellBrowser::QueryActiveShellView");

  ComPtr<IFolderView> folder_view;
  check_hr(shell_view->QueryInterface(IID_PPV_ARGS(folder_view.put())),
           "IShellView::QueryInterface(IFolderView)");
  return DesktopViewContext{std::move(shell_view), std::move(folder_view)};
}

ComPtr<IFolderView> desktop_folder_view() {
  auto context = desktop_view_context();
  return std::move(context.folder_view);
}

std::optional<fs::path> parsing_path_for_item(IShellFolder *folder,
                                              PCUITEMID_CHILD item) {
  STRRET strret{};
  HRESULT hr = folder->GetDisplayNameOf(item, SHGDN_FORPARSING, &strret);
  if (FAILED(hr)) {
    return std::nullopt;
  }
  wchar_t buffer[MAX_PATH * 4]{};
  hr =
      StrRetToBufW(&strret, item, buffer, static_cast<UINT>(std::size(buffer)));
  if (FAILED(hr)) {
    return std::nullopt;
  }
  return fs::path(buffer);
}

std::optional<fs::path> normalized_path(const fs::path &path) {
  std::error_code ec;
  fs::path value = fs::weakly_canonical(path, ec);
  if (!ec) {
    return value;
  }
  ec.clear();
  value = fs::absolute(path, ec).lexically_normal();
  if (!ec) {
    return value;
  }
  return std::nullopt;
}

bool path_equal_ci(const fs::path &a, const fs::path &b) {
  return to_lower_w(a.wstring()) == to_lower_w(b.wstring());
}

bool is_under_dir(const fs::path &child, const fs::path &parent,
                  bool strict = false) {
  auto c_opt = normalized_path(child);
  auto p_opt = normalized_path(parent);
  if (!c_opt || !p_opt) {
    return false;
  }

  const fs::path &c = *c_opt;
  const fs::path &p = *p_opt;
  if (path_equal_ci(c, p)) {
    return !strict;
  }

  auto cit = c.begin();
  auto pit = p.begin();
  for (; pit != p.end(); ++pit, ++cit) {
    if (cit == c.end()) {
      return false;
    }
    if (to_lower_w(cit->wstring()) != to_lower_w(pit->wstring())) {
      return false;
    }
  }
  return cit != c.end();
}

std::string relative_name_for_desktop_item(const fs::path &item_path,
                                           const fs::path &desktop) {
  if (!is_under_dir(item_path, desktop, true)) {
    return {};
  }
  std::error_code ec;
  fs::path rel = fs::relative(item_path, desktop, ec).lexically_normal();
  if (ec || rel.empty() || rel.is_absolute()) {
    return {};
  }
  for (const auto &part : rel) {
    if (part == L"..") {
      return {};
    }
  }
  return narrow(rel.wstring());
}

void save_layout(const Paths &p, const std::string &guid) {
  CoInit co;
  co.require();

  ComPtr<IFolderView> view = desktop_folder_view();
  ComPtr<IShellFolder> folder;
  check_hr(view->GetFolder(IID_PPV_ARGS(folder.put())),
           "IFolderView::GetFolder");

  ComPtr<IEnumIDList> items;
  check_hr(view->Items(SVGIO_ALLVIEW, IID_PPV_ARGS(items.put())),
           "IFolderView::Items");

  fs::create_directories(p.layouts);
  std::ofstream out(p.layout_file(guid), std::ios::binary | std::ios::trunc);
  if (!out) {
    throw AppError("Could not write layout file");
  }

  ITEMID_CHILD *item = nullptr;
  while (items->Next(1, &item, nullptr) == S_OK) {
    POINT pt{};
    HRESULT pos_hr = view->GetItemPosition(item, &pt);
    auto item_path = parsing_path_for_item(folder.get(), item);
    if (SUCCEEDED(pos_hr) && item_path &&
        is_under_dir(*item_path, p.desktop, true)) {
      std::string rel = relative_name_for_desktop_item(*item_path, p.desktop);
      if (!rel.empty()) {
        out << percent_encode(rel) << '\t' << pt.x << '\t' << pt.y << '\n';
      }
    }
    CoTaskMemFree(item);
    item = nullptr;
  }
}

std::map<std::string, POINT> load_layout(const fs::path &path,
                                         size_t *skipped_rows = nullptr) {
  std::map<std::string, POINT> result;
  std::ifstream in(path, std::ios::binary);
  if (!in) {
    return result;
  }

  std::string line;
  while (std::getline(in, line)) {
    std::istringstream iss(line);
    std::string encoded;
    std::string sx;
    std::string sy;
    if (std::getline(iss, encoded, '\t') && std::getline(iss, sx, '\t') &&
        std::getline(iss, sy, '\t')) {
      try {
        POINT pt{std::stoi(sx), std::stoi(sy)};
        result[percent_decode(encoded)] = pt;
      } catch (...) {
        if (skipped_rows) {
          ++*skipped_rows;
        }
      }
    } else if (skipped_rows) {
      ++*skipped_rows;
    }
  }
  return result;
}

void refresh_desktop(const Paths &p);

ComPtr<IFolderView2> desktop_folder_view2(IFolderView *view) {
  ComPtr<IFolderView2> view2;
  if (view) {
    view->QueryInterface(IID_PPV_ARGS(view2.put()));
  }
  return view2;
}

class FolderViewRedrawGuard {
public:
  explicit FolderViewRedrawGuard(IFolderView2 *view) : view_(view) {
    if (view_) {
      view_->SetRedraw(FALSE);
    }
  }

  ~FolderViewRedrawGuard() {
    if (view_) {
      view_->SetRedraw(TRUE);
    }
  }

  FolderViewRedrawGuard(const FolderViewRedrawGuard &) = delete;
  FolderViewRedrawGuard &operator=(const FolderViewRedrawGuard &) = delete;

private:
  IFolderView2 *view_ = nullptr;
};

std::map<std::string, ITEMID_CHILD *>
visible_desktop_items(IFolderView *view, IShellFolder *folder, const Paths &p) {
  std::map<std::string, ITEMID_CHILD *> result;
  ComPtr<IEnumIDList> items;
  check_hr(view->Items(SVGIO_ALLVIEW, IID_PPV_ARGS(items.put())),
           "IFolderView::Items");

  ITEMID_CHILD *item = nullptr;
  while (items->Next(1, &item, nullptr) == S_OK) {
    auto item_path = parsing_path_for_item(folder, item);
    if (item_path && is_under_dir(*item_path, p.desktop, true)) {
      std::string rel = relative_name_for_desktop_item(*item_path, p.desktop);
      if (!rel.empty()) {
        result[rel] = item;
        item = nullptr;
      }
    }
    if (item) {
      CoTaskMemFree(item);
      item = nullptr;
    }
  }

  return result;
}

void free_visible_items(std::map<std::string, ITEMID_CHILD *> &items) {
  for (auto &[_, item] : items) {
    CoTaskMemFree(item);
    item = nullptr;
  }
  items.clear();
}

std::map<std::string, ITEMID_CHILD *>
wait_for_layout_items(IShellView *shell_view, IFolderView *view,
                      IShellFolder *folder, const Paths &p,
                      const std::map<std::string, POINT> &layout) {
  auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
  std::map<std::string, ITEMID_CHILD *> visible;

  while (true) {
    if (shell_view) {
      shell_view->Refresh();
    }
    refresh_desktop(p);
    std::this_thread::sleep_for(std::chrono::milliseconds(100));

    free_visible_items(visible);
    visible = visible_desktop_items(view, folder, p);

    size_t matched = 0;
    for (const auto &[name, _] : layout) {
      if (visible.contains(name)) {
        ++matched;
      }
    }

    if (matched == layout.size() ||
        std::chrono::steady_clock::now() >= deadline) {
      return visible;
    }

    std::this_thread::sleep_for(std::chrono::milliseconds(250));
  }
}

void restore_layout(const Paths &p, const std::string &guid, bool verbose) {
  size_t skipped_rows = 0;
  auto layout = load_layout(p.layout_file(guid), &skipped_rows);
  if (skipped_rows > 0) {
    log_line(p,
             "ignored malformed layout rows: " + std::to_string(skipped_rows));
  }
  if (layout.empty()) {
    if (verbose) {
      std::cout << "No saved layout for " << guid << "\n";
    }
    return;
  }

  CoInit co;
  co.require();

  auto desktop_view = desktop_view_context();
  ComPtr<IFolderView> view = std::move(desktop_view.folder_view);
  ComPtr<IShellFolder> folder;
  check_hr(view->GetFolder(IID_PPV_ARGS(folder.put())),
           "IFolderView::GetFolder");

  ComPtr<IFolderView2> view2 = desktop_folder_view2(view.get());
  if (view2) {
    DWORD flags = 0;
    if (SUCCEEDED(view2->GetCurrentFolderFlags(&flags)) &&
        (flags & FWF_AUTOARRANGE)) {
      view2->SetCurrentFolderFlags(FWF_AUTOARRANGE, 0);
      if (verbose) {
        std::cout << "Disabled Desktop auto-arrange so saved icon positions "
                     "can be restored.\n";
      }
    }
  }

  std::map<std::string, ITEMID_CHILD *> visible = wait_for_layout_items(
      desktop_view.shell_view.get(), view.get(), folder.get(), p, layout);

  size_t initial_matches = 0;
  for (const auto &[rel, _] : layout) {
    if (visible.contains(rel)) {
      ++initial_matches;
    }
  }
  if (initial_matches == 0) {
    free_visible_items(visible);
    if (verbose) {
      std::cout << "Explorer desktop view had no matching saved items; "
                   "reacquiring the Shell view and retrying.\n";
    }
    desktop_view = desktop_view_context();
    view = std::move(desktop_view.folder_view);
    folder.reset();
    check_hr(view->GetFolder(IID_PPV_ARGS(folder.put())),
             "IFolderView::GetFolder");
    view2 = desktop_folder_view2(view.get());
    visible = wait_for_layout_items(desktop_view.shell_view.get(), view.get(),
                                    folder.get(), p, layout);
  }

  std::vector<PCUITEMID_CHILD> apidls;
  std::vector<POINT> positions;
  std::vector<std::string> missing;
  apidls.reserve(layout.size());
  positions.reserve(layout.size());

  for (const auto &[rel, pt] : layout) {
    auto found = visible.find(rel);
    if (found != visible.end()) {
      apidls.push_back(found->second);
      positions.push_back(pt);
    } else {
      missing.push_back(rel);
    }
  }

  HRESULT apply_hr = S_FALSE;
  {
    FolderViewRedrawGuard redraw_guard(view2.get());
    if (!apidls.empty()) {
      apply_hr = view->SelectAndPositionItems(static_cast<UINT>(apidls.size()),
                                              apidls.data(), positions.data(),
                                              SVSI_POSITIONITEM);
    }
  }

  free_visible_items(visible);
  refresh_desktop(p);

  if (verbose) {
    std::cout << "Matched " << apidls.size() << " of " << layout.size()
              << " saved icon positions for " << guid << "\n";
    if (!missing.empty()) {
      std::cout << "Missing from Explorer desktop view:\n";
      for (const auto &item : missing) {
        std::cout << "  " << item << "\n";
      }
    }
    if (apidls.empty()) {
      std::cout << "No positions were applied because no saved items matched "
                   "the current desktop view.\n";
    } else if (SUCCEEDED(apply_hr)) {
      std::cout << "Applied " << apidls.size() << " saved icon positions.\n";
    } else {
      std::cout << "SelectAndPositionItems failed: HRESULT 0x" << std::hex
                << static_cast<unsigned long>(apply_hr) << std::dec << "\n";
    }
  }
}

void dump_visible_items(const Paths &p) {
  CoInit co;
  co.require();

  ComPtr<IFolderView> view = desktop_folder_view();
  ComPtr<IShellFolder> folder;
  check_hr(view->GetFolder(IID_PPV_ARGS(folder.put())),
           "IFolderView::GetFolder");

  auto visible = visible_desktop_items(view.get(), folder.get(), p);
  std::cout << "Explorer desktop view items under managed user Desktop "
            << narrow(p.desktop.wstring()) << ":\n";
  for (const auto &[rel, item] : visible) {
    POINT pt{};
    if (SUCCEEDED(view->GetItemPosition(item, &pt))) {
      std::cout << "  " << rel << "\t" << pt.x << "\t" << pt.y << "\n";
    } else {
      std::cout << "  " << rel << "\t<no position>\n";
    }
  }
  free_visible_items(visible);
}

void refresh_desktop(const Paths &p) {
  const std::wstring user_desktop = p.desktop.wstring();
  const std::wstring public_desktop = p.public_desktop.wstring();
  SHChangeNotify(SHCNE_UPDATEDIR, SHCNF_PATHW, user_desktop.c_str(), nullptr);
  SHChangeNotify(SHCNE_UPDATEDIR, SHCNF_PATHW, public_desktop.c_str(), nullptr);
  SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, nullptr, nullptr);
}

void validate_moves(const std::vector<PlannedMove> &moves) {
  for (const auto &move : moves) {
    if (!fs::exists(move.from)) {
      throw AppError("Source disappeared before move: " +
                     narrow(move.from.wstring()));
    }
    if (fs::exists(move.to)) {
      throw AppError("Refusing to overwrite existing path: " +
                     narrow(move.to.wstring()));
    }
  }
}

void move_path(const fs::path &from, const fs::path &to) {
  fs::create_directories(to.parent_path());
  BOOL ok = MoveFileExW(from.wstring().c_str(), to.wstring().c_str(),
                        MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH);
  check_win32(ok != FALSE, "MoveFileExW " + narrow(from.wstring()) + " -> " +
                               narrow(to.wstring()));
}

void apply_moves(const std::vector<PlannedMove> &moves, bool dry_run,
                 const Paths *log_paths = nullptr) {
  validate_moves(moves);
  if (dry_run) {
    for (const auto &move : moves) {
      std::cout << "dry-run move: " << narrow(move.from.wstring()) << " -> "
                << narrow(move.to.wstring()) << "\n";
    }
    return;
  }

  std::vector<PlannedMove> completed;
  try {
    for (const auto &move : moves) {
      move_path(move.from, move.to);
      completed.push_back(move);
    }
  } catch (...) {
    for (auto it = completed.rbegin(); it != completed.rend(); ++it) {
      if (fs::exists(it->to) && !fs::exists(it->from)) {
        BOOL ok =
            MoveFileExW(it->to.wstring().c_str(), it->from.wstring().c_str(),
                        MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH);
        if (!ok && log_paths) {
          log_line(*log_paths,
                   "rollback move failed: " + narrow(it->to.wstring()) +
                       " -> " + narrow(it->from.wstring()) + ": " +
                       win32_error(GetLastError()));
        }
      }
    }
    throw;
  }
}

std::vector<PlannedMove> moves_from_to(const fs::path &from_dir,
                                       const fs::path &to_dir,
                                       const Config &config,
                                       const Paths *log_paths = nullptr) {
  std::vector<PlannedMove> moves;
  for (const auto &item : child_entries(from_dir, log_paths)) {
    if (!should_manage_entry(item, config)) {
      continue;
    }
    moves.push_back(PlannedMove{item, to_dir / item.filename()});
  }
  return moves;
}

void set_active_desktop(const Paths &p, const std::string &guid) {
  write_text_file(p.active_file, guid);
}

void adopt_current_desktop(const Paths &p, const std::string &current_guid) {
  ensure_dirs(p);
  fs::create_directories(p.set_files(current_guid));
  save_layout(p, current_guid);
  set_active_desktop(p, current_guid);
  log_line(p, "adopted current desktop " + current_guid);
}

void switch_to_current_desktop(const Paths &p, bool dry_run) {
  Config config = load_config(p);
  if (!config.enabled) {
    std::cout << "DeskIcons is disabled\n";
    return;
  }
  if (!dry_run) {
    recover_journal(p, true);
  }

  auto current_opt = current_virtual_desktop_guid();
  if (!current_opt) {
    throw AppError("Could not determine current virtual desktop GUID. Windows "
                   "virtual desktop registry keys may have changed.");
  }
  std::string current = guid_to_string(*current_opt);
  ensure_dirs(p);
  fs::create_directories(p.set_files(current));

  auto active_opt = read_text_file_trimmed(p.active_file);
  if (!active_opt) {
    std::cout << "No active DeskIcons state exists; adopting current desktop "
              << current << "\n";
    if (!dry_run) {
      adopt_current_desktop(p, current);
    }
    return;
  }

  std::string active = *active_opt;
  if (active == current) {
    if (!dry_run) {
      save_layout(p, current);
    }
    std::cout << "Already active on " << current << "\n";
    return;
  }

  fs::path active_files = p.set_files(active);
  fs::path target_files = p.set_files(current);
  fs::create_directories(active_files);
  fs::create_directories(target_files);

  std::cout << "Switching visible desktop icons\n";
  std::cout << "  from: " << active << "\n";
  std::cout << "  to:   " << current << "\n";
  log_line(p, "switch " + active + " -> " + current);

  if (!dry_run) {
    save_layout(p, active);
  }

  auto outbound = moves_from_to(p.desktop, active_files, config, &p);
  auto inbound = moves_from_to(target_files, p.desktop, config, &p);
  Journal journal{"planned", active, current, outbound, inbound};

  validate_moves(outbound);
  if (!dry_run) {
    write_journal(p, journal, "planned");
  }
  apply_moves(outbound, dry_run, &p);

  try {
    validate_moves(inbound);
    if (!dry_run) {
      write_journal(p, journal, "outbound-complete");
    }
    apply_moves(inbound, dry_run, &p);
  } catch (...) {
    if (!dry_run) {
      write_journal(p, journal, "rollback");
      std::vector<PlannedMove> rollback;
      for (const auto &move : outbound) {
        rollback.push_back(PlannedMove{move.to, move.from});
      }
      try {
        apply_moves(rollback, false, &p);
        set_active_desktop(p, active);
        clear_journal(p);
      } catch (const std::exception &ex) {
        log_error(p, ex);
        std::cerr << "Rollback failed; manual recovery may be required under "
                  << narrow(p.root.wstring()) << "\n";
      }
    }
    throw;
  }

  if (!dry_run) {
    write_journal(p, journal, "inbound-complete");
    set_active_desktop(p, current);
    refresh_desktop(p);
    std::this_thread::sleep_for(std::chrono::milliseconds(250));
    restore_layout(p, current, true);
    clear_journal(p);
    log_line(p, "switch complete " + active + " -> " + current);
  }
}

void print_status(const Paths &p) {
  Config config = load_config(p);
  std::cout << "DeskIcons status\n";
  std::cout << "  version:        " << kAppVersion << "\n";
  std::cout << "  user desktop:   " << narrow(p.desktop.wstring()) << "\n";
  std::cout << "  public desktop: " << narrow(p.public_desktop.wstring())
            << "\n";
  std::cout << "  managed scope:  user Desktop only; Public Desktop icons "
               "remain visible but unmanaged\n";
  std::cout << "  state root:     " << narrow(p.root.wstring()) << "\n";
  std::cout << "  enabled:        " << (app_enabled(p) ? "yes" : "no") << "\n";
  std::cout << "  startup:        " << (startup_enabled() ? "yes" : "no")
            << "\n";
  std::cout << "  manage files:   "
            << (config.manage_non_shortcuts ? "all user Desktop entries"
                                            : "user Desktop shortcuts only")
            << "\n";
  std::cout << "  poll ms:        " << config.poll_ms << "\n";
  std::cout << "  journal:        "
            << (fs::exists(p.journal_file) ? "pending" : "none") << "\n";

  std::wstring desktop_lower = to_lower_w(p.desktop.wstring());
  if (desktop_lower.find(L"onedrive") != std::wstring::npos) {
    std::cout
        << "  warning:        Desktop path appears to be OneDrive-synced\n";
  }

  std::cout << "  note:           virtual desktop detection uses undocumented "
               "Windows Explorer registry state\n";

  if (auto current = current_virtual_desktop_guid()) {
    std::cout << "  current VD:     " << guid_to_string(*current) << "\n";
  } else {
    std::cout << "  current VD:     <unknown>\n";
  }

  if (auto active = read_text_file_trimmed(p.active_file)) {
    std::cout << "  active set:     " << *active << "\n";
  } else {
    std::cout << "  active set:     <none>\n";
  }

  auto ids = virtual_desktop_ids();
  std::cout << "  known VDs:      " << ids.size() << "\n";
  for (size_t i = 0; i < ids.size(); ++i) {
    std::cout << "    [" << i << "] " << guid_to_string(ids[i]) << "\n";
  }
}

void run_agent(const Paths &p, bool dry_run) {
  ensure_dirs(p);
  log_line(p, std::string("agent start version ") + kAppVersion);
  if (!dry_run) {
    recover_journal(p, true);
  }
  if (!read_text_file_trimmed(p.active_file)) {
    if (auto current = current_virtual_desktop_guid()) {
      std::cout << "Initial adoption of current desktop "
                << guid_to_string(*current) << "\n";
      if (!dry_run) {
        adopt_current_desktop(p, guid_to_string(*current));
      }
    }
  }

  std::optional<std::string> last;
  std::cout << "DeskIcons agent running. Press Ctrl+C to stop.\n";
  while (true) {
    Config config = load_config(p);
    if (auto current = current_virtual_desktop_guid()) {
      std::string guid = guid_to_string(*current);
      if (!last) {
        last = guid;
      } else if (*last != guid && config.enabled) {
        switch_to_current_desktop(p, dry_run);
        last = guid;
      } else if (*last != guid) {
        last = guid;
      }
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(config.poll_ms));
  }
}

std::wstring exe_path() {
  std::wstring buffer(MAX_PATH, L'\0');
  DWORD size = GetModuleFileNameW(nullptr, buffer.data(),
                                  static_cast<DWORD>(buffer.size()));
  while (size == buffer.size()) {
    buffer.resize(buffer.size() * 2);
    size = GetModuleFileNameW(nullptr, buffer.data(),
                              static_cast<DWORD>(buffer.size()));
  }
  if (size == 0) {
    throw AppError("GetModuleFileNameW failed: " + win32_error(GetLastError()));
  }
  buffer.resize(size);
  return buffer;
}

fs::path exe_dir() { return fs::path(exe_path()).parent_path(); }

std::wstring startup_command() { return L"\"" + exe_path() + L"\" tray"; }

std::wstring startup_command_for(const fs::path &target_exe) {
  return L"\"" + target_exe.wstring() + L"\" tray";
}

bool startup_enabled() {
  DWORD type = 0;
  wchar_t buffer[4096]{};
  DWORD size = sizeof(buffer);
  LSTATUS status = RegGetValueW(
      HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run",
      kRunValueName, RRF_RT_REG_SZ, &type, buffer, &size);
  if (status != ERROR_SUCCESS || type != REG_SZ) {
    return false;
  }
  return std::wstring(buffer) == startup_command();
}

void set_startup_enabled_for_exe(const fs::path &target_exe, bool enabled) {
  UniqueHKey key;
  LSTATUS status = RegCreateKeyExW(
      HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run",
      0, nullptr, 0, KEY_SET_VALUE, nullptr, key.put(), nullptr);
  check_lstatus(status, "Could not open Run registry key");

  if (enabled) {
    std::wstring command = startup_command_for(target_exe);
    status = RegSetValueExW(
        key, kRunValueName, 0, REG_SZ,
        reinterpret_cast<const BYTE *>(command.c_str()),
        static_cast<DWORD>((command.size() + 1) * sizeof(wchar_t)));
  } else {
    status = RegDeleteValueW(key, kRunValueName);
    if (status == ERROR_FILE_NOT_FOUND) {
      status = ERROR_SUCCESS;
    }
  }
  check_lstatus(status, "Could not update startup registration");
}

void set_startup_enabled(bool enabled) {
  set_startup_enabled_for_exe(fs::path(exe_path()), enabled);
}

void open_path(const fs::path &path) {
  auto rc = reinterpret_cast<intptr_t>(
      ShellExecuteW(nullptr, L"open", path.wstring().c_str(), nullptr, nullptr,
                    SW_SHOWNORMAL));
  if (rc <= 32) {
    throw AppError("ShellExecuteW failed opening " + narrow(path.wstring()) +
                   ": code " + std::to_string(rc));
  }
}

constexpr wchar_t kUninstallRegKey[] =
    L"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\DeskIcons";

void create_start_menu_shortcut(const fs::path &shortcut_path,
                                const fs::path &target_exe) {
  CoInit co; // COM may have been uninitialized after paths() returned
  IShellLinkW *lnk = nullptr;
  if (FAILED(CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER,
                              IID_IShellLinkW,
                              reinterpret_cast<void **>(&lnk))))
    return;
  lnk->SetPath(target_exe.wstring().c_str());
  lnk->SetWorkingDirectory(target_exe.parent_path().wstring().c_str());
  lnk->SetDescription(L"DeskIcons");
  IPersistFile *pf = nullptr;
  if (SUCCEEDED(lnk->QueryInterface(IID_IPersistFile,
                                    reinterpret_cast<void **>(&pf)))) {
    pf->Save(shortcut_path.wstring().c_str(), TRUE);
    pf->Release();
  }
  lnk->Release();
}

void remove_start_menu_shortcut(const fs::path &shortcut_path) {
  std::error_code ec;
  fs::remove(shortcut_path, ec);
}

void register_uninstall_key(const Paths &p, const fs::path &target_exe) {
  HKEY hkey = nullptr;
  if (RegCreateKeyExW(HKEY_CURRENT_USER, kUninstallRegKey, 0, nullptr,
                      REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, nullptr, &hkey,
                      nullptr) != ERROR_SUCCESS) {
    return; // non-fatal
  }
  auto set_sz = [&](const wchar_t *name, std::wstring value) {
    RegSetValueExW(hkey, name, 0, REG_SZ,
                   reinterpret_cast<const BYTE *>(value.c_str()),
                   static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
  };
  auto set_dw = [&](const wchar_t *name, DWORD value) {
    RegSetValueExW(hkey, name, 0, REG_DWORD,
                   reinterpret_cast<const BYTE *>(&value), sizeof(DWORD));
  };
  std::wstring uninstall_str = L"\"" + target_exe.wstring() + L"\" uninstall";
  set_sz(L"DisplayName", L"DeskIcons");
  set_sz(L"DisplayVersion", widen(kAppVersion));
  set_sz(L"Publisher", L"DeskIcons");
  set_sz(L"InstallLocation", p.root.wstring());
  set_sz(L"UninstallString", uninstall_str);
  set_sz(L"DisplayIcon", target_exe.wstring());
  set_dw(L"NoModify", 1);
  set_dw(L"NoRepair", 1);
  RegCloseKey(hkey);
}

void remove_uninstall_key() {
  RegDeleteKeyW(HKEY_CURRENT_USER, kUninstallRegKey);
}

// Writes a temp .bat to %TEMP%, launches it detached, and returns. The bat
// kills all deskicons.exe instances and removes the install folder, then
// deletes itself. Uses %LOCALAPPDATA%\DeskIcons so no path-encoding issues.
void launch_uninstall_bat(const fs::path & /*install_root*/) {
  wchar_t temp_dir[MAX_PATH] = {};
  GetTempPathW(MAX_PATH, temp_dir);
  std::wstring bat_path = std::wstring(temp_dir) + L"deskicons_uninstall_" +
                          std::to_wstring(GetCurrentProcessId()) + L".bat";

  std::ofstream bat(bat_path, std::ios::binary | std::ios::trunc);
  if (!bat)
    return;
  bat << "@echo off\r\n";
  bat << "taskkill /f /im deskicons.exe >nul 2>&1\r\n";
  bat << "timeout /t 2 /nobreak >nul\r\n";
  bat << "rmdir /s /q \"%LOCALAPPDATA%\\DeskIcons\"\r\n";
  bat << "del /f /q \"%APPDATA%\\Microsoft\\Windows\\Start "
         "Menu\\Programs\\DeskIcons.lnk\" >nul 2>&1\r\n";
  bat << "echo.\r\n";
  bat << "if exist \"%LOCALAPPDATA%\\DeskIcons\" (\r\n";
  bat << "  echo Some files could not be deleted automatically.\r\n";
  bat << "  echo Please delete them manually from: "
         "%LOCALAPPDATA%\\DeskIcons\r\n";
  bat << ") else (\r\n";
  bat << "  echo DeskIcons uninstalled successfully.\r\n";
  bat << ")\r\n";
  bat << "echo.\r\n";
  bat << "timeout /t 5\r\n";
  bat << "del \"%~f0\"\r\n";
  bat.close();

  std::wstring cmdline = L"cmd.exe /c \"" + bat_path + L"\"";
  STARTUPINFOW si{};
  si.cb = sizeof(si);
  PROCESS_INFORMATION pi{};
  if (CreateProcessW(nullptr, cmdline.data(), nullptr, nullptr, FALSE,
                     CREATE_NEW_CONSOLE, nullptr, nullptr, &si, &pi)) {
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
  }
}

bool running_from_install_dir(const Paths &p) {
  auto current_dir = normalized_path(exe_dir());
  auto install_dir = normalized_path(p.root);
  return current_dir && install_dir &&
         path_equal_ci(*current_dir, *install_dir);
}

// Finds and terminates any deskicons.exe running from the install directory.
// Returns true if such an instance was found and killed.
bool installed_instance_running(const Paths &p) {
  auto install_dir = normalized_path(p.root);
  if (!install_dir)
    return false;

  HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
  if (snap == INVALID_HANDLE_VALUE)
    return false;

  bool found = false;
  PROCESSENTRY32W entry{};
  entry.dwSize = sizeof(entry);
  if (Process32FirstW(snap, &entry)) {
    do {
      if (_wcsicmp(entry.szExeFile, L"deskicons.exe") != 0)
        continue;
      HANDLE proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE,
                                entry.th32ProcessID);
      if (!proc)
        continue;
      wchar_t path_buf[MAX_PATH]{};
      DWORD size = MAX_PATH;
      if (QueryFullProcessImageNameW(proc, 0, path_buf, &size)) {
        auto proc_dir = normalized_path(fs::path(path_buf).parent_path());
        if (proc_dir && path_equal_ci(*proc_dir, *install_dir)) {
          found = true;
        }
      }
      CloseHandle(proc);
    } while (!found && Process32NextW(snap, &entry));
  }
  CloseHandle(snap);
  return found;
}

void kill_installed_instance(const Paths &p) {
  auto install_dir = normalized_path(p.root);
  if (!install_dir)
    return;

  HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
  if (snap == INVALID_HANDLE_VALUE)
    return;

  PROCESSENTRY32W entry{};
  entry.dwSize = sizeof(entry);
  if (Process32FirstW(snap, &entry)) {
    do {
      if (_wcsicmp(entry.szExeFile, L"deskicons.exe") != 0)
        continue;
      HANDLE proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION |
                                    PROCESS_TERMINATE | SYNCHRONIZE,
                                FALSE, entry.th32ProcessID);
      if (!proc)
        continue;
      wchar_t path_buf[MAX_PATH]{};
      DWORD size = MAX_PATH;
      if (QueryFullProcessImageNameW(proc, 0, path_buf, &size)) {
        auto proc_dir = normalized_path(fs::path(path_buf).parent_path());
        if (proc_dir && path_equal_ci(*proc_dir, *install_dir)) {
          TerminateProcess(proc, 0);
          WaitForSingleObject(proc, 5000);
        }
      }
      CloseHandle(proc);
    } while (Process32NextW(snap, &entry));
  }
  CloseHandle(snap);
}

bool show_install_dialog(bool is_update, bool &start_with_windows) {
  const wchar_t *button_label =
      is_update ? S().update_button : S().install_button;
  const wchar_t *window_title =
      is_update ? S().update_title : S().install_title;
  const wchar_t *main_instruction =
      is_update ? S().update_instruction : S().install_instruction;
  const wchar_t *content = is_update ? S().update_content : S().install_content;

  TASKDIALOG_BUTTON buttons[] = {
      {100, button_label},
      {IDCANCEL, S().cancel_button},
  };

  BOOL verification_checked = TRUE;
  int selected_button = IDCANCEL;
  TASKDIALOGCONFIG config{};
  config.cbSize = sizeof(config);
  config.hwndParent = nullptr;
  config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION;
  config.pszWindowTitle = window_title;
  config.pszMainInstruction = main_instruction;
  config.pszContent = content;
  config.cButtons = static_cast<UINT>(std::size(buttons));
  config.pButtons = buttons;
  config.nDefaultButton = 100;
  if (!is_update) {
    config.pszVerificationText = S().install_start_with_windows;
  }

  HRESULT hr = TaskDialogIndirect(&config, &selected_button, nullptr,
                                  &verification_checked);
  if (FAILED(hr)) {
    int result = MessageBoxW(
        nullptr,
        is_update ? S().update_fallback_content : S().install_fallback_content,
        window_title, MB_ICONQUESTION | MB_OKCANCEL | MB_DEFBUTTON1);
    start_with_windows = false;
    return result == IDOK;
  }

  start_with_windows = verification_checked != FALSE;
  return selected_button == 100;
}

bool install_and_restart_if_needed(const Paths &p) {
  if (running_from_install_dir(p)) {
    return false;
  }

  bool is_update = installed_instance_running(p);

  bool start_with_windows = true;
  if (!show_install_dialog(is_update, start_with_windows)) {
    return true;
  }

  if (is_update) {
    kill_installed_instance(p);
  }

  fs::create_directories(p.root);
  fs::path target_exe = p.root / L"deskicons.exe";
  fs::path old_exe = p.root / L"deskicons.exe.old";

  if (fs::exists(target_exe)) {
    std::error_code ec_rename;
    fs::rename(target_exe, old_exe, ec_rename);
  }
  // Attempt to make SmartScreen happy
  DeleteFileW((exe_path() + L":Zone.Identifier").c_str());

  DWORD copy_err = 0;
  bool copy_ok = false;
  for (int attempt = 0; attempt < 10; ++attempt) {
    SetLastError(0);
    copy_ok = CopyFileW(exe_path().c_str(), target_exe.wstring().c_str(),
                        /*failIfExists=*/FALSE) != 0;
    copy_err = GetLastError();
    if (copy_ok)
      break;
    Sleep(300);
  }

  if (copy_ok) {
    DeleteFileW((target_exe.wstring() + L":Zone.Identifier").c_str());
  }

  // Clean up old renamed exe (best-effort).
  if (fs::exists(old_exe)) {
    std::error_code ec_del;
    fs::remove(old_exe, ec_del);
  }

  if (!copy_ok) {
    std::wstring detail = widen(win32_error(copy_err)) +
                          std::wstring(L"\n\nError code: ") +
                          std::to_wstring(copy_err) + L"\nFrom: " + exe_path() +
                          L"\nTo:   " + target_exe.wstring();
    MessageBoxW(nullptr,
                (std::wstring(S().install_copy_error_prefix) + detail).c_str(),
                S().install_copy_error_title, MB_OK | MB_ICONERROR);
    return true;
  }

  if (!is_update && start_with_windows) {
    try {
      set_startup_enabled_for_exe(target_exe, true);
    } catch (const std::exception &ex) {
      MessageBoxW(nullptr, widen(ex.what()).c_str(), S().install_title,
                  MB_OK | MB_ICONWARNING);
    }
  }

  register_uninstall_key(p, target_exe);
  create_start_menu_shortcut(p.start_menu_shortcut, target_exe);
  write_text_file(p.install_notice_file, "1");

  std::wstring cmdline = L"\"" + target_exe.wstring() + L"\"";
  STARTUPINFOW si{};
  si.cb = sizeof(si);
  PROCESS_INFORMATION pi{};
  if (!CreateProcessW(target_exe.wstring().c_str(), cmdline.data(), nullptr,
                      nullptr, FALSE, 0, nullptr, p.root.wstring().c_str(), &si,
                      &pi)) {
    MessageBoxW(nullptr, S().install_started_error,
                is_update ? S().update_title : S().install_title,
                MB_OK | MB_ICONERROR);
  } else {
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
  }
  return true;
}

enum TrayCommand : UINT {
  ID_TRAY_ENABLE = 2001,
  ID_TRAY_ADOPT,
  ID_TRAY_RESTORE,
  ID_TRAY_RECOVER,
  ID_TRAY_OPEN_STATE,
  ID_TRAY_STARTUP,
  ID_TRAY_EXIT,
  ID_TRAY_LANG_BASE = 3000,
};

class TrayApp {
public:
  explicit TrayApp(Paths paths) : paths_(std::move(paths)) {}

  int run() {
    ensure_dirs(paths_);
    recover_journal(paths_, false);
    start_gdiplus();

    WNDCLASSW wc{};
    wc.lpfnWndProc = &TrayApp::window_proc;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = kTrayWindowClass;
    ATOM atom = RegisterClassW(&wc);
    if (atom == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
      throw AppError("RegisterClassW failed: " + win32_error(GetLastError()));
    }

    hwnd_ = CreateWindowExW(0, kTrayWindowClass, L"DeskIcons", WS_OVERLAPPED, 0,
                            0, 0, 0, nullptr, nullptr,
                            GetModuleHandleW(nullptr), this);
    check_win32(hwnd_ != nullptr, "CreateWindowExW");

    add_tray_icon();
    first_run_adopt();
    initialize_vd_state();
    start_vd_watcher();
    log_line(paths_, std::string("tray start version ") + kAppVersion);

    MSG msg{};
    while (GetMessageW(&msg, nullptr, 0, 0) > 0) {
      TranslateMessage(&msg);
      DispatchMessageW(&msg);
    }

    stop_vd_watcher();
    remove_tray_icon();
    stop_gdiplus();
    log_line(paths_, "tray exit");
    return 0;
  }

private:
  static LRESULT CALLBACK window_proc(HWND hwnd, UINT message, WPARAM wparam,
                                      LPARAM lparam) {
    TrayApp *app = nullptr;
    if (message == WM_NCCREATE) {
      auto *cs = reinterpret_cast<CREATESTRUCTW *>(lparam);
      app = reinterpret_cast<TrayApp *>(cs->lpCreateParams);
      SetWindowLongPtrW(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(app));
    } else {
      app = reinterpret_cast<TrayApp *>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
    }

    if (app) {
      return app->handle_message(hwnd, message, wparam, lparam);
    }
    return DefWindowProcW(hwnd, message, wparam, lparam);
  }

  LRESULT handle_message(HWND hwnd, UINT message, WPARAM wparam,
                         LPARAM lparam) {
    switch (message) {
    case WM_DESKICONS_VD_CHANGED:
      on_vd_changed();
      return 0;
    case WM_COMMAND:
      handle_command(LOWORD(wparam));
      return 0;
    case WM_DESKICONS_TRAY:
      switch (LOWORD(lparam)) {
      case WM_RBUTTONUP:
      case WM_CONTEXTMENU:
        show_menu();
        return 0;
      case WM_LBUTTONDBLCLK:
        open_path(paths_.root);
        return 0;
      default:
        break;
      }
      break;
    case WM_SETTINGCHANGE:
    case WM_THEMECHANGED:
      update_tray_icon();
      break;
    case WM_DESTROY:
      PostQuitMessage(0);
      return 0;
    default:
      break;
    }
    return DefWindowProcW(hwnd, message, wparam, lparam);
  }

  void start_gdiplus() {
    Gdiplus::GdiplusStartupInput input;
    if (Gdiplus::GdiplusStartup(&gdiplus_token_, &input, nullptr) !=
        Gdiplus::Ok) {
      gdiplus_token_ = 0;
      log_line(paths_, "GDI+ startup failed; using fallback tray icon");
    }
  }

  void stop_gdiplus() {
    tray_icon_.reset();
    if (gdiplus_token_) {
      Gdiplus::GdiplusShutdown(gdiplus_token_);
      gdiplus_token_ = 0;
    }
  }

  UniqueHIcon load_png_resource_icon(int resource_id) {
    if (!gdiplus_token_) {
      return {};
    }

    HMODULE module = GetModuleHandleW(nullptr);
    HRSRC resource =
        FindResourceW(module, MAKEINTRESOURCEW(resource_id), RT_RCDATA);
    if (!resource) {
      return {};
    }
    HGLOBAL loaded = LoadResource(module, resource);
    DWORD size = SizeofResource(module, resource);
    const void *data = loaded ? LockResource(loaded) : nullptr;
    if (!data || size == 0) {
      return {};
    }

    UniqueHGlobal copy(GlobalAlloc(GMEM_MOVEABLE, size));
    if (!copy.get()) {
      return {};
    }
    void *dest = GlobalLock(copy.get());
    if (!dest) {
      return {};
    }
    std::memcpy(dest, data, size);
    GlobalUnlock(copy.get());

    IStream *raw_stream = nullptr;
    if (FAILED(CreateStreamOnHGlobal(copy.get(), TRUE, &raw_stream)) ||
        !raw_stream) {
      return {};
    }
    copy.detach();
    ComPtr<IStream> stream(raw_stream);

    Gdiplus::Bitmap bitmap(stream.get());
    HICON icon = nullptr;
    if (bitmap.GetLastStatus() == Gdiplus::Ok &&
        bitmap.GetHICON(&icon) == Gdiplus::Ok && icon) {
      return UniqueHIcon(icon);
    }
    return {};
  }

  UniqueHIcon load_png_file_icon(const fs::path &icon_path) {
    if (gdiplus_token_ && fs::exists(icon_path)) {
      Gdiplus::Bitmap bitmap(icon_path.wstring().c_str());
      HICON icon = nullptr;
      if (bitmap.GetLastStatus() == Gdiplus::Ok &&
          bitmap.GetHICON(&icon) == Gdiplus::Ok && icon) {
        return UniqueHIcon(icon);
      }
    }
    return {};
  }

  HICON load_tray_icon() {
    UniqueHIcon icon = load_png_resource_icon(IDR_TRAY_PNG);
    if (!icon) {
      icon = load_png_file_icon(exe_dir() / L"icon.png");
    }
    if (icon) {
      tray_icon_ = std::move(icon);
      return tray_icon_.get();
    }
    return LoadIconW(nullptr, IDI_APPLICATION);
  }

  void update_tray_icon() {
    // Single icon — no theme switching needed.
  }

  void first_run_adopt() {
    bool show_install_notice = fs::exists(paths_.install_notice_file);
    if (read_text_file_trimmed(paths_.active_file)) {
      if (show_install_notice) {
        show_notification(S().notif_active_title, S().notif_installed);
        std::error_code ec;
        fs::remove(paths_.install_notice_file, ec);
      }
      return;
    }
    auto current = current_virtual_desktop_guid();
    if (!current) {
      log_line(paths_, "could not adopt first run because current virtual "
                       "desktop GUID was unavailable");
      return;
    }
    adopt_current_desktop(paths_, guid_to_string(*current));
    show_notification(S().notif_active_title, S().notif_adopted);
    if (show_install_notice) {
      std::error_code ec;
      fs::remove(paths_.install_notice_file, ec);
    }
  }

  void add_tray_icon() {
    NOTIFYICONDATAW nid{};
    nid.cbSize = sizeof(nid);
    nid.hWnd = hwnd_;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_DESKICONS_TRAY;
    nid.hIcon = load_tray_icon();
    wcscpy_s(nid.szTip, L"DeskIcons");
    check_win32(Shell_NotifyIconW(NIM_ADD, &nid) != FALSE,
                "Shell_NotifyIconW(NIM_ADD)");
    nid.uVersion = NOTIFYICON_VERSION_4;
    if (!Shell_NotifyIconW(NIM_SETVERSION, &nid)) {
      log_line(paths_, "Shell_NotifyIconW(NIM_SETVERSION) failed: " +
                           win32_error(GetLastError()));
    }
    tray_added_ = true;
  }

  void remove_tray_icon() {
    if (!hwnd_ || !tray_added_) {
      return;
    }
    NOTIFYICONDATAW nid{};
    nid.cbSize = sizeof(nid);
    nid.hWnd = hwnd_;
    nid.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &nid);
    tray_added_ = false;
  }

  void show_notification(const wchar_t *title, const wchar_t *text) {
    NOTIFYICONDATAW nid{};
    nid.cbSize = sizeof(nid);
    nid.hWnd = hwnd_;
    nid.uID = 1;
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    wcsncpy_s(nid.szInfoTitle, title, _TRUNCATE);
    wcsncpy_s(nid.szInfo, text, _TRUNCATE);
    if (!Shell_NotifyIconW(NIM_MODIFY, &nid)) {
      log_line(paths_, "Shell_NotifyIconW(NIM_MODIFY notification) failed: " +
                           win32_error(GetLastError()));
    }
  }

  void show_menu() {
    UniqueHMenu menu(CreatePopupMenu());
    if (!menu.h) {
      log_line(paths_,
               "CreatePopupMenu failed: " + win32_error(GetLastError()));
      return;
    }
    bool enabled = app_enabled(paths_);
    AppendMenuW(menu, MF_STRING | (enabled ? MF_CHECKED : 0), ID_TRAY_ENABLE,
                S().menu_enabled);
    AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
    // AppendMenuW(menu, MF_STRING, ID_TRAY_ADOPT, L"Adopt Current Desktop");
    AppendMenuW(menu, MF_STRING, ID_TRAY_RESTORE, S().menu_restore_layout);
    AppendMenuW(menu, MF_STRING, ID_TRAY_RECOVER, S().menu_recover);
    AppendMenuW(menu, MF_STRING, ID_TRAY_OPEN_STATE, S().menu_open_state);
    AppendMenuW(menu, MF_STRING | (startup_enabled() ? MF_CHECKED : 0),
                ID_TRAY_STARTUP, S().menu_startup);
    AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);

    HMENU lang_menu = CreatePopupMenu();
    if (lang_menu) {
      for (UINT i = 0; i < static_cast<UINT>(std::size(kLanguages)); ++i) {
        UINT flags =
            MF_STRING | (kLanguages[i].lang == g_language ? MF_CHECKED : 0);
        AppendMenuW(lang_menu, flags, ID_TRAY_LANG_BASE + i,
                    kLanguages[i].label);
      }
      // Parent menu takes ownership of lang_menu after this call.
      AppendMenuW(menu, MF_POPUP | MF_STRING,
                  reinterpret_cast<UINT_PTR>(lang_menu), S().menu_language);
    }

    AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
    AppendMenuW(menu, MF_STRING, ID_TRAY_EXIT, S().menu_exit);

    POINT pt{};
    if (!GetCursorPos(&pt)) {
      log_line(paths_, "GetCursorPos failed: " + win32_error(GetLastError()));
      return;
    }
    SetForegroundWindow(hwnd_);
    if (!TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0,
                        hwnd_, nullptr)) {
      log_line(paths_, "TrackPopupMenu failed: " + win32_error(GetLastError()));
    }
  }

  void handle_command(UINT id) {
    try {
      switch (id) {
      case ID_TRAY_ENABLE:
        set_enabled(paths_, !app_enabled(paths_));
        break;
      case ID_TRAY_ADOPT:
        if (auto current = current_virtual_desktop_guid()) {
          adopt_current_desktop(paths_, guid_to_string(*current));
          MessageBoxW(hwnd_, S().msg_adopt_ok, S().msg_title,
                      MB_OK | MB_ICONINFORMATION);
        } else {
          MessageBoxW(hwnd_, S().msg_adopt_error, S().msg_title,
                      MB_OK | MB_ICONERROR);
        }
        break;
      case ID_TRAY_RESTORE:
        if (auto current = current_virtual_desktop_guid()) {
          restore_layout(paths_, guid_to_string(*current), false);
        }
        break;
      case ID_TRAY_RECOVER:
        if (!recover_journal(paths_, true)) {
          MessageBoxW(hwnd_, S().msg_recover_nothing, S().msg_title,
                      MB_OK | MB_ICONINFORMATION);
        }
        break;
      case ID_TRAY_OPEN_STATE:
        open_path(paths_.root);
        break;
      case ID_TRAY_STARTUP: {
        bool target = !startup_enabled();
        set_startup_enabled(target);
        log_line(paths_, target ? "enabled startup" : "disabled startup");
        break;
      }
      case ID_TRAY_EXIT:
        DestroyWindow(hwnd_);
        break;
      default:
        if (id >= ID_TRAY_LANG_BASE &&
            id < static_cast<UINT>(ID_TRAY_LANG_BASE + std::size(kLanguages))) {
          size_t idx = id - ID_TRAY_LANG_BASE;
          Config config = load_config(paths_);
          config.language = kLanguages[idx].lang;
          save_config(paths_, config);
          set_language(kLanguages[idx].lang);
          log_line(paths_,
                   std::string("language changed to ") + kLanguages[idx].code);
        }
        break;
      }
    } catch (const std::exception &ex) {
      log_error(paths_, ex);
      MessageBoxW(hwnd_, widen(ex.what()).c_str(), S().msg_error_title,
                  MB_OK | MB_ICONERROR);
    }
  }

  void on_vd_changed() {
    try {
      if (!app_enabled(paths_)) {
        return;
      }
      auto current = current_virtual_desktop_guid();
      if (!current) {
        return;
      }
      std::string guid = guid_to_string(*current);
      if (!last_guid_) {
        last_guid_ = guid;
        switch_to_current_desktop(paths_, false);
      } else if (*last_guid_ != guid) {
        switch_to_current_desktop(paths_, false);
        //show_notification(S().notif_switched_title, S().notif_switched);
        last_guid_ = guid;
      }
    } catch (const std::exception &ex) {
      log_error(paths_, ex);
    }
  }

  void initialize_vd_state() {
    try {
      if (!app_enabled(paths_)) {
        return;
      }
      auto current = current_virtual_desktop_guid();
      if (!current) {
        return;
      }
      last_guid_ = guid_to_string(*current);
      switch_to_current_desktop(paths_, false);
    } catch (const std::exception &ex) {
      log_error(paths_, ex);
    }
  }

  void vd_watcher_thread() {
    constexpr auto kMainKey =
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer"
        L"\\VirtualDesktops";

    DWORD session_id = 0;
    ProcessIdToSessionId(GetCurrentProcessId(), &session_id);
    std::wstring session_key =
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer"
        L"\\SessionInfo\\" +
        std::to_wstring(session_id) + L"\\VirtualDesktops";

    // Two auto-reset events, one per key. Both stay valid for the thread's
    // lifetime so RegNotifyChangeKeyValue can signal them asynchronously.
    UniqueHandle main_event(
        CreateEventW(nullptr, FALSE, FALSE, nullptr)); // auto-reset
    UniqueHandle sess_event(
        CreateEventW(nullptr, FALSE, FALSE, nullptr)); // auto-reset
    if (!main_event)
      return;

    while (true) {
      // Open keys and register notifications.
      // The key handle must stay open until the event fires, so UniqueHKey
      // lives for the full iteration.
      UniqueHKey main_key;
      bool main_ok =
          RegOpenKeyExW(HKEY_CURRENT_USER, kMainKey, 0, KEY_NOTIFY,
                        main_key.put()) == ERROR_SUCCESS &&
          RegNotifyChangeKeyValue(main_key.h, FALSE, REG_NOTIFY_CHANGE_LAST_SET,
                                  main_event.h, TRUE) == ERROR_SUCCESS;

      UniqueHKey sess_key;
      bool sess_ok =
          sess_event &&
          RegOpenKeyExW(HKEY_CURRENT_USER, session_key.c_str(), 0, KEY_NOTIFY,
                        sess_key.put()) == ERROR_SUCCESS &&
          RegNotifyChangeKeyValue(sess_key.h, FALSE, REG_NOTIFY_CHANGE_LAST_SET,
                                  sess_event.h, TRUE) == ERROR_SUCCESS;

      if (!main_ok && !sess_ok) {
        // Keys not present yet; wait briefly and retry.
        if (WaitForSingleObject(stop_event_.h, 2000) == WAIT_OBJECT_0)
          break;
        continue;
      }

      HANDLE handles[3] = {stop_event_.h, main_event.h,
                           sess_ok ? sess_event.h : stop_event_.h};
      DWORD count = sess_ok ? 3u : 2u;

      DWORD wait = WaitForMultipleObjects(count, handles, FALSE, INFINITE);
      // UniqueHKey destructors run here, closing key handles after the wait.

      if (wait == WAIT_OBJECT_0)
        break; // stop event signalled
      if (wait == WAIT_OBJECT_0 + 1 || wait == WAIT_OBJECT_0 + 2)
        PostMessageW(hwnd_, WM_DESKICONS_VD_CHANGED, 0, 0);
      // Loop to re-register for the next change.
    }
  }

  void start_vd_watcher() {
    stop_event_.reset(CreateEventW(nullptr, TRUE, FALSE, nullptr));
    watcher_thread_ = std::thread([this] { vd_watcher_thread(); });
  }

  void stop_vd_watcher() {
    if (stop_event_.h)
      SetEvent(stop_event_.h);
    if (watcher_thread_.joinable())
      watcher_thread_.join();
    stop_event_.reset();
  }

  Paths paths_;
  HWND hwnd_ = nullptr;
  bool tray_added_ = false;
  ULONG_PTR gdiplus_token_ = 0;
  UniqueHIcon tray_icon_;
  std::optional<std::string> last_guid_;
  UniqueHandle stop_event_;
  std::thread watcher_thread_;
};

fs::path export_state(const Paths &p) {
  ensure_dirs(p);
  std::string stamp = timestamp_now();
  std::replace(stamp.begin(), stamp.end(), ':', '-');
  std::replace(stamp.begin(), stamp.end(), ' ', '_');
  fs::path dest = p.exports / widen("deskicons-state-" + stamp);
  fs::create_directories(dest);

  auto copy_if_exists = [](const fs::path &from, const fs::path &to) {
    std::error_code ec;
    if (!fs::exists(from, ec)) {
      return;
    }
    if (fs::is_directory(from, ec)) {
      fs::copy(from, to,
               fs::copy_options::recursive |
                   fs::copy_options::overwrite_existing,
               ec);
    } else {
      fs::create_directories(to.parent_path(), ec);
      fs::copy_file(from, to, fs::copy_options::overwrite_existing, ec);
    }
  };

  copy_if_exists(p.config_file, dest / L"config.ini");
  copy_if_exists(p.active_file, dest / L"active-desktop.txt");
  copy_if_exists(p.journal_file, dest / L"swap.journal");
  copy_if_exists(p.layouts, dest / L"layouts");
  copy_if_exists(p.logs, dest / L"logs");

  std::ofstream manifest(dest / L"manifest.txt",
                         std::ios::binary | std::ios::trunc);
  if (!manifest) {
    throw AppError("Could not write export manifest");
  }
  manifest << "DeskIcons " << kAppVersion << "\n";
  manifest << "exported=" << timestamp_now() << "\n";
  manifest << "user_desktop=" << narrow(p.desktop.wstring()) << "\n";
  manifest << "public_desktop=" << narrow(p.public_desktop.wstring()) << "\n";
  manifest << "managed_scope=user_desktop_only\n";
  manifest << "virtual_desktop_source=undocumented_explorer_registry_state\n";
  if (auto current = current_virtual_desktop_guid()) {
    manifest << "current_virtual_desktop=" << guid_to_string(*current) << "\n";
  }
  manifest << "startup_enabled=" << (startup_enabled() ? "1" : "0") << "\n";
  return dest;
}

bool has_arg(const std::vector<std::string> &args, std::string_view value) {
  return std::find(args.begin(), args.end(), value) != args.end();
}

void print_usage() {
  std::cout << "DeskIcons " << kAppVersion
            << "\n"
               "\n"
               "Usage:\n"
               "  deskicons status\n"
               "  deskicons adopt --yes\n"
               "  deskicons switch-once [--dry-run]\n"
               "  deskicons restore-layout --yes\n"
               "  deskicons dump-visible\n"
               "  deskicons recover\n"
               "  deskicons enable|disable\n"
               "  deskicons startup on|off\n"
               "  deskicons export-state\n"
               "  deskicons tray\n"
               "  deskicons agent [--dry-run]\n"
               "\n"
               "Notes:\n"
               "  status is read-only.\n"
               "  adopt records the current virtual desktop as the owner of "
               "current user Desktop items.\n"
               "  switch-once swaps user Desktop folder contents if Windows is "
               "now on another virtual desktop.\n"
               "  restore-layout reapplies the saved icon positions for the "
               "current virtual desktop.\n"
               "  dump-visible prints Explorer's current user Desktop item "
               "names and positions.\n"
               "  recover completes or rolls back an interrupted journaled "
               "swap according to journal stage.\n"
               "  tray runs the tray UI.\n"
               "  agent polls the current virtual desktop and runs switch-once "
               "on desktop changes.\n"
               "  Public Desktop icons remain visible but unmanaged.\n"
               "  Virtual desktop detection depends on undocumented Windows "
               "Explorer registry state.\n";
}

void attach_parent_console() {
  HANDLE out = GetStdHandle(STD_OUTPUT_HANDLE);
  if (out && out != INVALID_HANDLE_VALUE &&
      GetFileType(out) != FILE_TYPE_UNKNOWN) {
    return;
  }

  if (!AttachConsole(ATTACH_PARENT_PROCESS)) {
    return;
  }

  FILE *stream = nullptr;
  freopen_s(&stream, "CONOUT$", "w", stdout);
  freopen_s(&stream, "CONOUT$", "w", stderr);
  freopen_s(&stream, "CONIN$", "r", stdin);
  std::ios::sync_with_stdio();
}

} // namespace

int main(int argc, char **argv) {
  try {
    std::vector<std::string> args(argv + 1, argv + argc);
    if (args.empty()) {
      Paths p = paths();
      set_language(load_config(p).language.value_or(detect_system_language()));
      if (install_and_restart_if_needed(p)) {
        return 0;
      }
      TrayApp app(p);
      return app.run();
    }

    if (args[0] != "tray") {
      attach_parent_console();
    }

    if (args[0] == "help" || args[0] == "--help" || args[0] == "-h") {
      print_usage();
      return 0;
    }

    Paths p = paths();
    set_language(load_config(p).language.value_or(detect_system_language()));
    const std::string &command = args[0];
    bool dry_run = has_arg(args, "--dry-run");

    if (command == "status") {
      print_status(p);
    } else if (command == "adopt") {
      if (!has_arg(args, "--yes")) {
        throw AppError("adopt requires --yes");
      }
      auto current = current_virtual_desktop_guid();
      if (!current) {
        throw AppError(
            "Could not determine current virtual desktop GUID. Windows virtual "
            "desktop registry keys may have changed.");
      }
      adopt_current_desktop(p, guid_to_string(*current));
      std::cout << "Adopted current desktop set " << guid_to_string(*current)
                << "\n";
    } else if (command == "switch-once") {
      switch_to_current_desktop(p, dry_run);
    } else if (command == "restore-layout") {
      if (!has_arg(args, "--yes")) {
        throw AppError("restore-layout requires --yes");
      }
      auto current = current_virtual_desktop_guid();
      if (!current) {
        throw AppError(
            "Could not determine current virtual desktop GUID. Windows virtual "
            "desktop registry keys may have changed.");
      }
      restore_layout(p, guid_to_string(*current), true);
    } else if (command == "dump-visible") {
      dump_visible_items(p);
    } else if (command == "recover") {
      if (!recover_journal(p, true)) {
        std::cout << "No interrupted swap journal exists.\n";
      }
    } else if (command == "enable") {
      set_enabled(p, true);
      std::cout << "DeskIcons enabled.\n";
    } else if (command == "disable") {
      set_enabled(p, false);
      std::cout << "DeskIcons disabled.\n";
    } else if (command == "startup") {
      if (args.size() < 2 || (args[1] != "on" && args[1] != "off")) {
        throw AppError("startup requires 'on' or 'off'");
      }
      set_startup_enabled(args[1] == "on");
      std::cout << "Startup " << (startup_enabled() ? "enabled" : "disabled")
                << ".\n";
    } else if (command == "export-state") {
      fs::path dest = export_state(p);
      std::cout << "Exported state to " << narrow(dest.wstring()) << "\n";
    } else if (command == "uninstall") {
      // Remove registry entries first (can be done from within the process).
      try {
        set_startup_enabled_for_exe(p.root / L"deskicons.exe", false);
      } catch (...) {
      }
      remove_uninstall_key();
      // Write and launch a temp bat that kills any running instance and
      // deletes the install folder after this process exits.
      launch_uninstall_bat(p.root);
    } else if (command == "tray") {
      if (install_and_restart_if_needed(p)) {
        return 0;
      }
      TrayApp app(p);
      return app.run();
    } else if (command == "agent") {
      run_agent(p, dry_run);
    } else {
      print_usage();
      return 2;
    }

    return 0;
  } catch (const std::exception &ex) {
    std::cerr << "error: " << ex.what() << "\n";
    return 1;
  }
}
