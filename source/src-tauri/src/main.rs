#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const TASK_APP: &str = "EmzLinkApp";
const TASK_BYPASS: &str = "EmzLinkBypass";
const TASK_TG: &str = "EmzLinkTgProxy";
const TG_RELEASE_API: &str = "https://api.github.com/repos/Flowseal/tg-ws-proxy/releases/latest";
const IPSET_URL: &str =
    "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/ipset-service.txt";

static TESTING: AtomicBool = AtomicBool::new(false);
static CANCEL_TEST: AtomicBool = AtomicBool::new(false);
static LAUNCHING: AtomicBool = AtomicBool::new(false);
const CONFLICT_MSG: &str = "CONFLICT: уже запущен другой обход (zapret или прокси). Откройте проверку совместимости.";
static UI_READY: AtomicBool = AtomicBool::new(false);
static LAUNCH_LOCK: Mutex<()> = Mutex::new(());
static INTENT_BYPASS: AtomicBool = AtomicBool::new(false);
static INTENT_TG: AtomicBool = AtomicBool::new(false);
const LATEST_VERSION_URL: &str =
    "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/.service/version.txt";
const ARCHIVE_URL: &str = "https://github.com/Flowseal/zapret-discord-youtube/archive/refs/tags/";

// ───────────────────────── settings ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Default)]
struct AltResult {
    status: String,
    ms: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct TweakCfg {
    tcp: bool,
    throttle: bool,
    tcpset: bool,
    power: bool,
    nicpower: bool,
    gamedvr: bool,
    dopt: bool,
    mtu: String,
    dns: String,
}

impl Default for TweakCfg {
    fn default() -> Self {
        TweakCfg { tcp: true, throttle: true, tcpset: true, power: true, nicpower: true, gamedvr: true, dopt: true, mtu: "1500".into(), dns: "cloudflare".into() }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Settings {
    zapret_dir: String,
    zapret_dir_custom: bool,
    style: String,
    mode: String,
    accent: String,
    strategy: String,
    bypass_autostart: bool,
    app_autostart: bool,
    notify: bool,
    auto_heal: bool,
    tweaks: TweakCfg,
    services: HashMap<String, bool>,
    custom_domains: Vec<String>,
    tg_proxy: bool,
    tg_autostart: bool,
    experimental: bool,
    transparency: bool,
    alt_results: HashMap<String, AltResult>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            zapret_dir: String::new(),
            zapret_dir_custom: false,
            style: "win11".into(),
            mode: "system".into(),
            accent: "#0067C0".into(),
            strategy: "general (ALT).bat".into(),
            bypass_autostart: false,
            app_autostart: false,
            notify: true,
            auto_heal: false,
            tweaks: TweakCfg::default(),
            services: HashMap::new(),
            custom_domains: Vec::new(),
            tg_proxy: false,
            tg_autostart: false,
            experimental: false,
            transparency: true,
            alt_results: HashMap::new(),
        }
    }
}

struct AppState {
    settings: Mutex<Settings>,
    start_hidden: bool,
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("com.emzlink.app").join("settings.json")
}

static SAVE_LOCK: Mutex<()> = Mutex::new(());

fn load_settings() -> Settings {
    let path = settings_path();
    let parsed: Option<Settings> = match fs::read_to_string(&path) {
        Ok(t) => match serde_json::from_str(t.trim_start_matches('\u{feff}')) {
            Ok(v) => Some(v),
            Err(_) => {
                // keep the broken file for inspection instead of silently losing it
                let _ = fs::copy(&path, path.with_extension("json.bad"));
                None
            }
        },
        Err(_) => None,
    };
    let mut s: Settings = parsed.unwrap_or_default();
    let resolved = resolve_zapret_dir(&s.zapret_dir, s.zapret_dir_custom)
        .to_string_lossy()
        .to_string();
    let changed = resolved != s.zapret_dir;
    s.zapret_dir = resolved;
    set_our_dir(Path::new(&s.zapret_dir));
    if changed {
        let _ = save_settings(&s);
    }
    s
}

/// Written to a temp file first and then renamed, so a crash or two threads can't leave half a file.
fn save_settings(s: &Settings) -> Result<(), String> {
    let _g = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = settings_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, &text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).or_else(|_| fs::write(&p, &text)).map_err(|e| e.to_string())
}

fn log_line(msg: &str) {
    use std::io::Write;
    let Some(base) = std::env::var_os("APPDATA") else { return };
    let dir = PathBuf::from(base).join("com.emzlink.app");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("emzlink.log");
    if fs::metadata(&path).map(|m| m.len() > 200_000).unwrap_or(false) {
        let _ = fs::remove_file(&path);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{}] {}", secs, msg);
    }
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// An EmzLink folder: zapret lives either in it or in its "zapret" subfolder.
fn has_zapret(dir: &Path) -> bool {
    is_installed(dir) || is_installed(&dir.join("zapret"))
}

fn is_installed(dir: &Path) -> bool {
    dir.join("bin").join("winws.exe").is_file() && dir.join("service.bat").is_file()
}

/// Next to EmzLink.exe if zapret is there, otherwise an ASCII-only folder in ProgramData
/// (the .bat files of zapret do not like Cyrillic / special characters in the path).
fn default_zapret_dir() -> PathBuf {
    let here = exe_dir();
    let sub = here.join("zapret");
    if is_installed(&sub) {
        return sub;
    }
    if is_installed(&here) {
        return here;
    }
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
        .join("EmzLink")
        .join("zapret")
}

fn programdata_zapret_dir() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
        .join("EmzLink")
        .join("zapret")
}

/// Finds the folder that really contains zapret. A path saved in settings.json can go stale
/// (folder renamed / moved / unpacked elsewhere), so the real location always wins:
/// 1) a folder the user picked by hand, 2) the folder of EmzLink.exe, 3) its "zapret" subfolder,
/// 4) the previously saved path, 5) ProgramData\EmzLink\zapret.
fn resolve_zapret_dir(saved: &str, custom: bool) -> PathBuf {
    let saved = saved.trim();
    let saved_p = PathBuf::from(saved);
    let here = exe_dir();
    let mut cands: Vec<PathBuf> = Vec::new();
    if custom && !saved.is_empty() {
        cands.push(saved_p.clone());
    }
    cands.push(here.join("zapret"));
    cands.push(here.clone());
    if !saved.is_empty() {
        cands.push(saved_p.clone());
    }
    cands.push(programdata_zapret_dir());
    for c in &cands {
        if is_installed(c) {
            return c.clone();
        }
    }
    if custom && !saved.is_empty() {
        return saved_p;
    }
    default_zapret_dir()
}

/// The picked folder itself, or one of its direct subfolders (e.g. "zapret-discord-youtube-main").
fn find_zapret_in(dir: &Path) -> Option<PathBuf> {
    if is_installed(dir) {
        return Some(dir.to_path_buf());
    }
    for e in fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() && is_installed(&p) {
            return Some(p);
        }
    }
    None
}

/// Human-readable reason why a folder is not usable (shown in the app).
fn install_problem(dir: &Path) -> String {
    if !dir.exists() {
        "Папка не найдена".into()
    } else if !dir.join("bin").join("winws.exe").is_file() {
        "Нет файла bin\\winws.exe — возможно, его удалил антивирус".into()
    } else if !dir.join("service.bat").is_file() {
        "Нет файла service.bat".into()
    } else if list_strategies(dir).is_empty() {
        "Нет файлов general*.bat".into()
    } else {
        String::new()
    }
}

// ───────────────────────── process helpers ─────────────────────────

fn quiet(c: &mut Command) -> &mut Command {
    c.stdin(Stdio::null());
    #[cfg(windows)]
    {
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn powershell(script: &str, envs: &[(&str, &str)], sta: bool) -> Result<String, String> {
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass"]);
    if sta {
        c.arg("-STA");
    }
    let full = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ProgressPreference='SilentlyContinue'; try {{ [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12 }} catch {{}}; {}",
        script
    );
    c.args(["-Command", &full]);
    for (k, v) in envs {
        c.env(k, v);
    }
    quiet(&mut c);
    let out = c.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(clean_ps_error(&if err.is_empty() { stdout } else { err }))
    }
}

/// All running processes as (pid, exe name) — native snapshot, no child processes spawned.
#[cfg(windows)]
fn process_list() -> Vec<(u32, String)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let mut out: Vec<(u32, String)> = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut e) != 0 {
            loop {
                let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                out.push((e.th32ProcessID, String::from_utf16_lossy(&e.szExeFile[..len])));
                if Process32NextW(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
fn process_list() -> Vec<(u32, String)> {
    Vec::new()
}

fn process_ids(name: &str) -> Vec<u32> {
    process_list()
        .into_iter()
        .filter(|(_, n)| n.eq_ignore_ascii_case(name))
        .map(|(p, _)| p)
        .collect()
}

/// Full path of the exe behind a pid (None if it can't be read).
#[cfg(windows)]
fn image_path(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok != 0 {
            Some(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
fn image_path(_pid: u32) -> Option<PathBuf> {
    None
}

fn norm_path(p: &Path) -> String {
    p.to_string_lossy().trim_start_matches("\\\\?\\").replace('/', "\\").to_lowercase()
}

fn same_path(a: &Path, b: &Path) -> bool {
    norm_path(a) == norm_path(b)
}

/// The zapret folder EmzLink works with; used to tell "our" winws.exe from somebody else's.
static OUR_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

fn set_our_dir(dir: &Path) {
    *OUR_DIR.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_path_buf());
}

fn our_winws() -> Option<PathBuf> {
    OUR_DIR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|d| d.join("bin").join("winws.exe"))
}

/// Folder with the ASCII-only junctions to zapret folders whose real path has spaces or non-Latin letters.
fn run_links_base() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
        .join("EmzLink")
        .join("run")
}

/// zapret's .bat files break on spaces and non-Latin letters in the path. For such a folder winws is started
/// through a junction with a plain ASCII path (no admin rights needed, nothing is copied).
fn ascii_run_dir(dir: &Path) -> PathBuf {
    let text = dir.to_string_lossy().to_string();
    if text.is_ascii() && !text.contains(' ') {
        return dir.to_path_buf();
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut h: u64 = 0xcbf29ce484222325;
        for b in norm_path(dir).bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let link = run_links_base().join(format!("z{:x}", h));
        if link.join("bin").join("winws.exe").is_file() {
            return link;
        }
        let _ = fs::create_dir_all(run_links_base());
        let _ = fs::remove_dir(&link); // stale junction
        let mut c = Command::new("cmd.exe");
        c.raw_arg(format!("/c mklink /J \"{}\" \"{}\"", link.display(), dir.display()));
        quiet(&mut c);
        let _ = c.output();
        if link.join("bin").join("winws.exe").is_file() {
            return link;
        }
        log_line(&format!("could not create a junction for {}", text));
    }
    dir.to_path_buf()
}

/// (ours, foreign) winws.exe processes. A process whose path can't be read counts as ours.
fn winws_split() -> (Vec<u32>, Vec<(u32, String)>) {
    let ours_path = our_winws();
    let (mut ours, mut foreign) = (Vec::new(), Vec::new());
    for pid in process_ids("winws.exe") {
        match (image_path(pid), &ours_path) {
            (Some(p), Some(o)) if same_path(&p, o) || norm_path(&p).starts_with(&norm_path(&run_links_base())) => ours.push(pid),
            (Some(p), _) => foreign.push((pid, p.to_string_lossy().to_string())),
            (None, _) => ours.push(pid),
        }
    }
    (ours, foreign)
}

fn foreign_winws() -> Vec<(u32, String)> {
    winws_split().1
}

/// True when OUR winws.exe is running (what the UI shows as "on").
fn is_running() -> bool {
    !winws_split().0.is_empty()
}

fn count_ours() -> usize {
    winws_split().0.len()
}


/// Kills OUR winws only (used between restarts; the WinDivert driver stays loaded).
/// Somebody else's winws.exe is never touched here — see resolve_conflicts for that.
fn kill_winws() {
    let ours = winws_split().0;
    if ours.is_empty() {
        return;
    }
    for pid in &ours {
        let mut c = Command::new("taskkill");
        c.args(["/F", "/T", "/PID", &pid.to_string()]);
        quiet(&mut c);
        let _ = c.output();
    }
    for _ in 0..60 {
        if winws_split().0.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn windivert_state(name: &str) -> String {
    let mut c = Command::new("sc");
    c.args(["query", name]);
    quiet(&mut c);
    c.output().map(|o| String::from_utf8_lossy(&o.stdout).to_uppercase()).unwrap_or_default()
}

/// Stops the WinDivert service and waits until it really is stopped; with `delete` the stale service entry
/// is removed afterwards (winws registers a fresh one for its own folder). Deleting a service that is still
/// loaded leaves it "marked for deletion" until reboot, so deletion only happens after a confirmed stop.
fn reset_windivert(delete: bool) {
    for drv in ["WinDivert", "WinDivert14"] {
        let before = windivert_state(drv);
        if before.contains("1060") || before.is_empty() {
            continue; // not installed
        }
        run_quiet("sc", &["stop", drv]);
        let mut stopped = false;
        for _ in 0..40 {
            let st = windivert_state(drv);
            if st.contains("STOPPED") || st.contains("1060") {
                stopped = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        if delete && stopped {
            run_quiet("sc", &["delete", drv]);
        }
    }
}

/// Full stop: winws and the WinDivert driver.
fn stop_winws() {
    kill_winws();
    for svc in ["WinDivert", "WinDivert14"] {
        let mut s = Command::new("sc");
        s.args(["stop", svc]);
        quiet(&mut s);
        let _ = s.output();
    }
}

/// Hides the console window of winws and raises its priority a bit (fewer queued packets = lower latency).
/// Done natively (no PowerShell): instant and no extra process.
#[cfg(windows)]
fn tune_once() {
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{OpenProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS, PROCESS_SET_INFORMATION};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId, ShowWindow, SW_HIDE};

    let pids = process_ids("winws.exe");
    if pids.is_empty() {
        return;
    }
    unsafe {
        for &pid in &pids {
            let h = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
            if !h.is_null() {
                SetPriorityClass(h, ABOVE_NORMAL_PRIORITY_CLASS);
                CloseHandle(h);
            }
        }
        unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> i32 {
            let pids = &*(lp as *const Vec<u32>);
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut buf = [0u16; 128];
            let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            let title = if n > 0 { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() };
            // the .bat names its console "zapret: <name>"
            if pids.contains(&pid) || title.starts_with("zapret:") {
                ShowWindow(hwnd, SW_HIDE);
            }
            1
        }
        EnumWindows(Some(cb), &pids as *const Vec<u32> as LPARAM);
    }
}

#[cfg(not(windows))]
fn tune_once() {}

fn tune_winws() {
    tune_once();
    // the console window can appear a moment later than the process
    thread::spawn(|| {
        thread::sleep(Duration::from_millis(700));
        tune_once();
    });
}

fn valid_strategy_name(file: &str) -> bool {
    file.to_lowercase().ends_with(".bat")
        && !file.contains('/')
        && !file.contains('\\')
        && !file.contains("..")
        && file.to_lowercase().starts_with("general")
}

/// Named mutex shared with the headless "--run-strategy" process started by the autostart task,
/// so the task and the app can never launch winws at the same time.
#[cfg(windows)]
struct ProcLock(*mut core::ffi::c_void);
#[cfg(windows)]
impl ProcLock {
    fn acquire() -> Option<ProcLock> {
        use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
        let name: Vec<u16> = "Local\\EmzLinkLaunch".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if h.is_null() {
                return None;
            }
            WaitForSingleObject(h, 30_000);
            Some(ProcLock(h))
        }
    }
}
#[cfg(windows)]
impl Drop for ProcLock {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::ReleaseMutex;
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
struct ProcLock;
#[cfg(not(windows))]
impl ProcLock {
    fn acquire() -> Option<ProcLock> {
        None
    }
}

struct LaunchGuard;
impl LaunchGuard {
    fn new() -> Self {
        LAUNCHING.store(true, Ordering::SeqCst);
        LaunchGuard
    }
}
impl Drop for LaunchGuard {
    fn drop(&mut self) {
        LAUNCHING.store(false, Ordering::SeqCst);
    }
}

fn launch_strategy(dir: &Path, file: &str, settings: &Settings) -> Result<(), String> {
    launch_strategy_ex(dir, file, settings, true)
}

fn launch_strategy_ex(dir: &Path, file: &str, settings: &Settings, tune: bool) -> Result<(), String> {
    // one launch at a time: overlapping restarts used to leave two winws.exe running (latency spikes)
    let _lock = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _proc_lock = ProcLock::acquire();
    let _guard = LaunchGuard::new();
    set_our_dir(dir);
    if !foreign_winws().is_empty() {
        return Err(CONFLICT_MSG.into());
    }
    if !is_installed(dir) {
        return Err("zapret не найден в выбранной папке".into());
    }
    if !valid_strategy_name(file) {
        return Err("Некорректное имя режима".into());
    }
    let bat = dir.join(file);
    if !bat.is_file() {
        return Err(format!("Файл {} не найден", file));
    }
    let run_dir = ascii_run_dir(dir);
    let run_bat = prepare_run(&run_dir, &run_dir.join(file), settings)?;

    for attempt in 0..2 {
        kill_winws();
        let mut c = Command::new("cmd.exe");
        c.current_dir(&run_dir)
            .env("NO_UPDATE_CHECK", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            c.raw_arg(format!("/c \"\"{}\"\"", run_bat.display()));
        }
        quiet(&mut c);
        c.spawn().map_err(|e| e.to_string())?;

        let mut up = false;
        for _ in 0..120 {
            thread::sleep(Duration::from_millis(100));
            if is_running() {
                up = true;
                break;
            }
        }
        if !up {
            log_line(&format!("winws did not start ({}), attempt {}", file, attempt + 1));
            if attempt == 0 {
                // a stale WinDivert service (old folder, crashed run) is the usual reason: reset it and retry once
                kill_winws();
                reset_windivert(true);
                continue;
            }
            return Err("Не удалось запустить обход. Закройте антивирус на минуту или перезагрузите ПК, затем повторите.".into());
        }
        thread::sleep(Duration::from_millis(400));
        if count_ours() > 1 {
            continue; // duplicate instance: kill everything and start again
        }
        if tune {
            tune_winws();
        }
        return Ok(());
    }
    Err("Обход запустился дважды подряд. Закройте лишние копии winws.exe и повторите.".into())
}

// ───────────────────────── services / lists ─────────────────────────

struct SvcDef {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    domains: &'static [&'static str],
}

const SERVICES: &[SvcDef] = &[
    SvcDef { id: "youtube", name: "YouTube", group: "main", domains: &[] },
    SvcDef { id: "discord", name: "Discord", group: "main", domains: &[] },
    SvcDef { id: "twitch", name: "Twitch", group: "exp", domains: &["twitch.tv", "www.twitch.tv", "api.twitch.tv", "passport.twitch.tv", "gql.twitch.tv", "usher.ttvnw.net", "ttvnw.net", "jtvnw.net", "twitchcdn.net", "live-video.net"] },
    SvcDef { id: "steam", name: "Steam", group: "exp", domains: &["steampowered.com", "store.steampowered.com", "steamcommunity.com", "steamstatic.com", "steam-chat.com", "steamcontent.com", "steamusercontent.com", "steamserver.net", "valvesoftware.com"] },
    SvcDef { id: "hltv", name: "HLTV", group: "exp", domains: &["hltv.org", "www.hltv.org", "static.hltv.org", "img.hltv.org", "api.hltv.org", "scorebot.hltv.org", "esl.com", "hltv-live.akamaized.net"] },
    SvcDef { id: "modrinth", name: "Modrinth", group: "exp", domains: &["modrinth.com", "www.modrinth.com", "api.modrinth.com", "cdn.modrinth.com", "staging-api.modrinth.com", "knossos.modrinth.com"] },
    SvcDef { id: "roblox", name: "Roblox", group: "exp", domains: &["roblox.com", "www.roblox.com", "rbxcdn.com", "robloxlabs.com", "setup.rbxcdn.com"] },
    SvcDef { id: "epic", name: "Epic Games", group: "exp", domains: &["epicgames.com", "store.epicgames.com", "unrealengine.com", "ol.epicgames.com", "tracking.epicgames.com"] },
    SvcDef { id: "x", name: "X (Twitter)", group: "exp", domains: &["x.com", "twitter.com", "twimg.com", "t.co", "abs.twimg.com", "pbs.twimg.com", "video.twimg.com"] },
];

#[derive(Serialize)]
struct SvcOut {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    hint: &'static str,
}

fn svc_on(s: &Settings, id: &str) -> bool {
    let default = id == "youtube" || id == "discord";
    *s.services.get(id).unwrap_or(&default)
}

fn clean_domain(raw: &str) -> Option<String> {
    let mut d = raw.trim().to_lowercase();
    for p in ["https://", "http://"] {
        if let Some(r) = d.strip_prefix(p) {
            d = r.to_string();
        }
    }
    let d = d.split(['/', '?', '#', ':']).next().unwrap_or("").trim_start_matches("www.").to_string();
    let ok = d.contains('.')
        && d.len() <= 253
        && !d.starts_with('.')
        && !d.ends_with('.')
        && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if ok { Some(d) } else { None }
}

const BLOCK_BEGIN: &str = "# >>> EmzLink (managed block, edit via the app)";
const BLOCK_END: &str = "# <<< EmzLink";

fn read_lines(p: &Path) -> Vec<String> {
    fs::read(p)
        .map(|b| {
            String::from_utf8_lossy(&b)
                .trim_start_matches('\u{feff}')
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn write_lines(p: &Path, lines: &[String]) -> Result<(), String> {
    let mut t = lines.join("\r\n");
    t.push_str("\r\n");
    fs::write(p, t).map_err(|e| e.to_string())
}

/// Builds emz-list-general.txt / emz-list-google.txt (filtered copies of the stock lists)
/// and refreshes the EmzLink block inside list-general-user.txt.
fn prepare_lists(dir: &Path, s: &Settings) -> Result<(), String> {
    let lists = dir.join("lists");
    fs::create_dir_all(&lists).map_err(|e| e.to_string())?;
    let yt = svc_on(s, "youtube");
    let dc = svc_on(s, "discord");

    let yt_words = ["youtube", "youtu.be", "ytimg", "ggpht", "googlevideo", "googleusercontent", "googleapis", "play.google", "l.google.com"];
    let filter = |lines: Vec<String>| -> Vec<String> {
        let mut out: Vec<String> = lines
            .into_iter()
            .filter(|l| {
                let t = l.trim().to_lowercase();
                if t.is_empty() {
                    return false;
                }
                if t.starts_with('#') {
                    return true;
                }
                let is_dc = t.contains("discord");
                let is_yt = yt_words.iter().any(|w| t.contains(w));
                !((is_dc && !dc) || (is_yt && !yt))
            })
            .collect();
        if !out.iter().any(|l| !l.trim().starts_with('#')) {
            out.push("emz-placeholder.invalid".into());
        }
        out
    };
    write_lines(&lists.join("emz-list-general.txt"), &filter(read_lines(&lists.join("list-general.txt"))))?;
    write_lines(&lists.join("emz-list-google.txt"), &filter(read_lines(&lists.join("list-google.txt"))))?;

    // user list with the managed block
    let user_path = lists.join("list-general-user.txt");
    let mut keep: Vec<String> = Vec::new();
    let mut inside = false;
    for l in read_lines(&user_path) {
        if l.trim() == BLOCK_BEGIN {
            inside = true;
            continue;
        }
        if l.trim() == BLOCK_END {
            inside = false;
            continue;
        }
        if !inside {
            keep.push(l);
        }
    }
    let mut domains: Vec<String> = Vec::new();
    for d in SERVICES.iter().filter(|d| d.group == "exp" && svc_on(s, d.id)) {
        domains.extend(d.domains.iter().map(|x| x.to_string()));
    }
    for c in &s.custom_domains {
        if let Some(d) = clean_domain(c) {
            domains.push(d);
        }
    }
    domains.sort();
    domains.dedup();
    if !domains.is_empty() {
        keep.push(BLOCK_BEGIN.into());
        keep.extend(domains);
        keep.push(BLOCK_END.into());
    }
    if !keep.iter().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#')
    }) {
        keep.push("domain.example.abc".into());
    }
    write_lines(&user_path, &keep)
}

/// Copy of the chosen .bat that uses the filtered lists.
fn prepare_run(dir: &Path, bat: &Path, s: &Settings) -> Result<PathBuf, String> {
    prepare_lists(dir, s)?;
    let text = String::from_utf8_lossy(&fs::read(bat).map_err(|e| e.to_string())?).to_string();
    let text = text
        .replace("list-general.txt", "emz-list-general.txt")
        .replace("list-google.txt", "emz-list-google.txt");
    let out = dir.join(".emz_run.bat");
    fs::write(&out, text).map_err(|e| e.to_string())?;
    Ok(out)
}

// ───────────────────────── zapret options (game filter / ipset) ─────────────────────────

fn game_mode_get(dir: &Path) -> String {
    for l in read_lines(&dir.join("utils").join("game_filter.enabled")) {
        let l = l.trim().to_lowercase();
        if let Some(v) = l.strip_prefix("mode=") {
            return match v.trim() {
                "all" | "tcp" | "udp" => v.trim().to_string(),
                _ => "off".into(),
            };
        }
        if l == "all" || l == "tcp" || l == "udp" {
            return l;
        }
    }
    "off".into()
}

fn game_mode_set(dir: &Path, mode: &str) -> Result<(), String> {
    let p = dir.join("utils").join("game_filter.enabled");
    match mode {
        "off" => {
            let _ = fs::remove_file(&p);
            Ok(())
        }
        "all" | "tcp" | "udp" => {
            fs::create_dir_all(dir.join("utils")).map_err(|e| e.to_string())?;
            fs::write(&p, format!("mode={}\r\ntcp=1024-65535\r\nudp=1024-65535\r\n", mode)).map_err(|e| e.to_string())
        }
        _ => Err("Неизвестный режим".into()),
    }
}

fn ipset_status(dir: &Path) -> String {
    let p = dir.join("lists").join("ipset-all.txt");
    let lines: Vec<String> = read_lines(&p).into_iter().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        "any".into()
    } else if lines.iter().any(|l| l.trim() == "203.0.113.113/32") {
        "none".into()
    } else {
        "loaded".into()
    }
}

fn ipset_download(dir: &Path) -> Result<(), String> {
    let dest = dir.join("lists").join("ipset-all.txt");
    let script = format!(
        "$ErrorActionPreference='Stop'; Invoke-WebRequest -Uri '{}' -OutFile $env:EMZ_DEST -UseBasicParsing -TimeoutSec 30",
        IPSET_URL
    );
    powershell(&script, &[("EMZ_DEST", &dest.to_string_lossy())], false)
        .map(|_| ())
        .map_err(|_| "Не удалось скачать список IPSet. Проверьте интернет.".to_string())
}

fn ipset_set(dir: &Path, mode: &str) -> Result<(), String> {
    let lists = dir.join("lists");
    fs::create_dir_all(&lists).map_err(|e| e.to_string())?;
    let file = lists.join("ipset-all.txt");
    let backup = lists.join("ipset-all.txt.backup");
    let cur = ipset_status(dir);
    if cur == mode {
        return Ok(());
    }
    if cur == "loaded" {
        let _ = fs::remove_file(&backup);
        let _ = fs::rename(&file, &backup);
    }
    match mode {
        "none" => fs::write(&file, "203.0.113.113/32\r\n").map_err(|e| e.to_string()),
        "any" => fs::write(&file, "").map_err(|e| e.to_string()),
        "loaded" => {
            if backup.is_file() {
                let _ = fs::remove_file(&file);
                fs::rename(&backup, &file).map_err(|e| e.to_string())
            } else {
                ipset_download(dir)
            }
        }
        _ => Err("Неизвестный режим".into()),
    }
}

// ───────────────────────── strategies ─────────────────────────

#[derive(Serialize)]
struct Strategy {
    file: String,
    name: String,
}

fn pretty_name(file: &str) -> String {
    let stem = file.trim_end_matches(".bat").trim_end_matches(".BAT");
    let rest = stem
        .strip_prefix("general")
        .or_else(|| stem.strip_prefix("General"))
        .unwrap_or(stem)
        .trim();
    let rest = rest.trim_start_matches('(').trim_end_matches(')').trim();
    if rest.is_empty() {
        return "Стандартный".into();
    }
    // "ALT6" -> "ALT 6"
    let digits = rest.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < rest.len() {
        let split = rest.len() - digits;
        let (a, b) = rest.split_at(split);
        if a.chars().last().map(|c| c.is_alphabetic()).unwrap_or(false) {
            return format!("{} {}", a, b);
        }
    }
    rest.to_string()
}

fn sort_key(name: &str) -> (u8, String, u32) {
    let upper = name.to_uppercase();
    let digits: String = upper
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let base = upper.trim_end_matches(|c: char| c.is_ascii_digit()).trim().to_string();
    let rank = if upper == "СТАНДАРТНЫЙ" {
        0
    } else if base == "ALT" {
        1
    } else if base.starts_with("SIMPLE") {
        2
    } else if base.starts_with("FAKE") {
        3
    } else {
        4
    };
    (rank, base, digits.parse().unwrap_or(0))
}

fn list_strategies(dir: &Path) -> Vec<Strategy> {
    let mut v: Vec<Strategy> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|f| valid_strategy_name(f))
                .map(|f| Strategy {
                    name: pretty_name(&f),
                    file: f,
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| sort_key(&s.name));
    v
}

// ───────────────────────── autostart (Task Scheduler) ─────────────────────────

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn task_xml(command: &str, args: &str, workdir: &str, level: &str) -> String {
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(d), Ok(u)) => format!("<UserId>{}\\{}</UserId>", xml_esc(&d), xml_esc(&u)),
        _ => String::new(),
    };
    let args_xml = if args.is_empty() {
        String::new()
    } else {
        format!("<Arguments>{}</Arguments>", xml_esc(args))
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>EmzLink</Description></RegistrationInfo>
  <Triggers>
    <LogonTrigger><Enabled>true</Enabled>{user}<Delay>PT8S</Delay></LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>{level}</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      {args_xml}
      <WorkingDirectory>{workdir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>"#,
        user = user,
        level = level,
        command = xml_esc(command),
        args_xml = args_xml,
        workdir = xml_esc(workdir)
    )
}

fn create_task(name: &str, xml: &str) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("{}.xml", name));
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let mut c = Command::new("schtasks");
    c.args(["/Create", "/TN", name, "/XML"]).arg(&path).arg("/F");
    quiet(&mut c);
    let out = c.output().map_err(|e| e.to_string());
    let _ = fs::remove_file(&path);
    let out = out?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Не удалось создать задачу автозапуска: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn delete_task(name: &str) {
    let mut c = Command::new("schtasks");
    c.args(["/Delete", "/TN", name, "/F"]);
    quiet(&mut c);
    let _ = c.output();
}

fn task_exists(name: &str) -> bool {
    let mut c = Command::new("schtasks");
    c.args(["/Query", "/TN", name]);
    quiet(&mut c);
    c.output().map(|o| o.status.success()).unwrap_or(false)
}

fn register_app_task() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let xml = task_xml(
        &exe.to_string_lossy(),
        "--minimized",
        &exe_dir().to_string_lossy(),
        "HighestAvailable",
    );
    create_task(TASK_APP, &xml)
}

fn register_bypass_task(strategy: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let xml = task_xml(
        &exe.to_string_lossy(),
        &format!("--run-strategy \"{}\"", strategy),
        &exe_dir().to_string_lossy(),
        "HighestAvailable",
    );
    create_task(TASK_BYPASS, &xml)
}

// ───────────────────────── Telegram proxy (tg-ws-proxy) ─────────────────────────

fn tg_dir() -> PathBuf {
    exe_dir().join("tgproxy")
}

fn tg_exe() -> PathBuf {
    tg_dir().join("TgWsProxy.exe")
}

fn tg_cfg_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("TgWsProxy")
}

fn is_tg_name(n: &str) -> bool {
    n.to_ascii_lowercase().starts_with("tgwsproxy") && n.to_ascii_lowercase().ends_with(".exe")
}

/// (ours, foreign) TgWsProxy processes (official builds are named TgWsProxy.exe or TgWsProxy_windows.exe).
fn tg_split() -> (Vec<u32>, Vec<(u32, String)>) {
    let ours_path = tg_exe();
    let (mut ours, mut foreign) = (Vec::new(), Vec::new());
    for (pid, name) in process_list() {
        if !is_tg_name(&name) {
            continue;
        }
        match image_path(pid) {
            Some(p) if same_path(&p, &ours_path) => ours.push(pid),
            Some(p) => foreign.push((pid, p.to_string_lossy().to_string())),
            None => foreign.push((pid, name)),
        }
    }
    (ours, foreign)
}

fn tg_running() -> bool {
    !tg_split().0.is_empty()
}

fn tg_stop() {
    for pid in tg_split().0 {
        let mut c = Command::new("taskkill");
        c.args(["/F", "/T", "/PID", &pid.to_string()]);
        quiet(&mut c);
        let _ = c.output();
    }
}

fn random_hex16() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Makes sure the proxy has a config (so the secret stays stable) and skips its first-run window.
fn tg_seed() -> Result<(), String> {
    let dir = tg_cfg_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let marker = dir.join(".first_run_done_mtproto");
    if !marker.exists() {
        let _ = fs::write(&marker, "");
    }
    let cfg = dir.join("config.json");
    if !cfg.exists() {
        let text = format!(
            "{{\n  \"host\": \"127.0.0.1\",\n  \"port\": 1443,\n  \"secret\": \"{}\"\n}}\n",
            random_hex16()
        );
        fs::write(cfg, text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn tg_link() -> String {
    let v: serde_json::Value = fs::read_to_string(tg_cfg_dir().join("config.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::Value::Null);
    let host = v.get("host").and_then(|x| x.as_str()).unwrap_or("127.0.0.1");
    let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
    let port = v.get("port").and_then(|x| x.as_u64()).unwrap_or(1443);
    match v.get("secret").and_then(|x| x.as_str()) {
        Some(sec) if !sec.is_empty() => format!("tg://proxy?server={}&port={}&secret=dd{}", host, port, sec),
        _ => String::new(),
    }
}

fn tg_install() -> Result<String, String> {
    let dest = tg_exe();
    let script = r#"
$ErrorActionPreference='Stop'
[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
$h=@{'User-Agent'='EmzLink'}
$r=Invoke-RestMethod -Uri $env:EMZ_API -Headers $h -UseBasicParsing
$a=$r.assets | Where-Object { $_.name -eq 'TgWsProxy_windows.exe' } | Select-Object -First 1
if(-not $a){ throw 'asset not found' }
New-Item -ItemType Directory -Path (Split-Path $env:EMZ_DEST) -Force | Out-Null
Invoke-WebRequest -Uri $a.browser_download_url -OutFile $env:EMZ_DEST -UseBasicParsing
Write-Output $r.tag_name
"#;
    let tag = powershell(
        script,
        &[("EMZ_API", TG_RELEASE_API), ("EMZ_DEST", &dest.to_string_lossy())],
        false,
    )
    .map_err(|_| "Не удалось скачать прокси. Проверьте интернет.".to_string())?;
    let _ = fs::write(tg_dir().join("version.txt"), &tag);
    Ok(tag)
}

fn tg_start() -> Result<(), String> {
    let exe = tg_exe();
    if !exe.is_file() {
        return Err("Прокси не установлен".into());
    }
    if tg_running() {
        return Ok(());
    }
    if !tg_split().1.is_empty() {
        return Err("CONFLICT: уже запущен другой TgWsProxy".into());
    }
    tg_seed()?;
    // explorer starts it without admin rights (EmzLink itself runs elevated)
    let mut c = Command::new("explorer.exe");
    c.arg(&exe);
    c.spawn().map_err(|e| e.to_string())?;
    for _ in 0..40 {
        thread::sleep(Duration::from_millis(250));
        if tg_running() {
            return Ok(());
        }
    }
    Err("Прокси не запустился".into())
}

fn register_tg_task() -> Result<(), String> {
    let xml = task_xml(
        &tg_exe().to_string_lossy(),
        "",
        &tg_dir().to_string_lossy(),
        "LeastPrivilege",
    );
    create_task(TASK_TG, &xml)
}

#[derive(Serialize)]
struct TgDto {
    installed: bool,
    running: bool,
    link: String,
    version: String,
}

fn tg_dto() -> TgDto {
    TgDto {
        installed: tg_exe().is_file(),
        running: tg_running(),
        link: tg_link(),
        version: fs::read_to_string(tg_dir().join("version.txt")).unwrap_or_default().trim().to_string(),
    }
}

// ───────────────────────── mode testing ─────────────────────────

fn curl_available() -> bool {
    std::env::var_os("SystemRoot")
        .map(|r| PathBuf::from(r).join("System32").join("curl.exe").is_file())
        .unwrap_or(false)
}

fn probe(urls: &[&str]) -> (usize, u32) {
    let handles: Vec<_> = urls
        .iter()
        .map(|u| {
            let u = u.to_string();
            thread::spawn(move || -> Option<u32> {
                let mut c = Command::new("curl.exe");
                c.args([
                    "-s", "-o", "NUL", "-I", "-m", "4", "--ssl-no-revoke", "-w",
                    "%{http_code} %{time_appconnect}",
                ])
                .arg(&u);
                quiet(&mut c);
                let out = c.output().ok()?;
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                let mut it = text.split_whitespace();
                let code: u32 = it.next()?.parse().ok()?;
                let secs: f64 = it.next()?.parse().ok()?;
                if code >= 100 {
                    Some((secs * 1000.0) as u32)
                } else {
                    None
                }
            })
        })
        .collect();
    let times: Vec<u32> = handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect();
    let n = times.len();
    let avg = if n > 0 { times.iter().sum::<u32>() / n as u32 } else { 0 };
    (n, avg)
}

#[derive(Serialize, Clone)]
struct TestEvent {
    file: String,
    status: String,
    ms: u32,
    index: usize,
    total: usize,
}

#[derive(Serialize, Clone)]
struct TestDone {
    running: bool,
    picked: Option<String>,
    quick: bool,
}

fn test_urls(s: &Settings) -> Vec<&'static str> {
    let mut urls: Vec<&'static str> = Vec::new();
    if svc_on(s, "youtube") {
        urls.extend(["https://www.youtube.com", "https://i.ytimg.com"]);
    }
    if svc_on(s, "discord") {
        urls.extend(["https://discord.com", "https://cdn.discordapp.com"]);
    }
    if urls.is_empty() {
        urls.extend(["https://www.youtube.com", "https://discord.com"]);
    }
    urls
}

/// Most promising modes first: known good (fastest first), then mid, then untested, then known bad.
fn quick_order(files: Vec<String>, res: &HashMap<String, AltResult>) -> Vec<String> {
    let rank = |f: &String| match res.get(f).map(|r| r.status.as_str()) {
        Some("good") => 0,
        Some("mid") => 1,
        None => 2,
        _ => 3,
    };
    let mut v = files;
    v.sort_by_key(|f| (rank(f), res.get(f).map(|r| r.ms).unwrap_or(0)));
    v
}

/// quick = stop at the first fully working mode and select it (auto-pick);
/// otherwise every mode is measured (full check). auto = started by the watchdog, not by the user.
fn run_tests(app: AppHandle, quick: bool, auto: bool) {
    let a = app.clone();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_tests_inner(a, quick, auto)));
    if r.is_err() {
        // never leave the app stuck in "testing" after an unexpected error
        log_line("run_tests panicked");
        TESTING.store(false, Ordering::SeqCst);
        INTENT_BYPASS.store(false, Ordering::SeqCst);
        let _ = app.emit("test-done", TestDone { running: false, picked: None, quick });
    }
}

fn run_tests_inner(app: AppHandle, quick: bool, auto: bool) {
    log_line(&format!("test start quick={} auto={}", quick, auto));
    INTENT_BYPASS.store(false, Ordering::SeqCst);
    let (dir, current, settings, was_running) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock().unwrap().clone();
        (PathBuf::from(&s.zapret_dir), s.strategy.clone(), s, is_running())
    };
    let mut files: Vec<String> = list_strategies(&dir).into_iter().map(|x| x.file).collect();
    if quick {
        files = quick_order(files, &settings.alt_results);
    }
    let total = files.len();
    let urls = test_urls(&settings);

    let mut picked: Option<String> = None;
    let mut best_mid: Option<(String, u32)> = None;
    let mut last_launched: Option<String> = None;

    for (i, file) in files.iter().enumerate() {
        if CANCEL_TEST.load(Ordering::SeqCst) {
            break;
        }
        let _ = app.emit(
            "test-progress",
            TestEvent { file: file.clone(), status: "testing".into(), ms: 0, index: i, total },
        );
        let (status, ms) = match launch_strategy_ex(&dir, file, &settings, false) {
            Ok(()) => {
                last_launched = Some(file.clone());
                thread::sleep(Duration::from_millis(1200));
                let (ok, avg) = probe(&urls);
                if ok == urls.len() {
                    ("good".to_string(), avg)
                } else if ok > 0 {
                    ("mid".to_string(), avg)
                } else {
                    ("bad".to_string(), 0)
                }
            }
            Err(_) => ("bad".to_string(), 0),
        };
        {
            let st = app.state::<AppState>();
            let mut s = st.settings.lock().unwrap();
            s.alt_results.insert(file.clone(), AltResult { status: status.clone(), ms });
            let _ = save_settings(&s);
        }
        log_line(&format!("test {} -> {} {} ms", file, status, ms));
        let _ = app.emit("test-result", TestEvent { file: file.clone(), status: status.clone(), ms, index: i, total });
        if quick {
            if status == "good" {
                picked = Some(file.clone());
                break;
            }
            if status == "mid" && best_mid.as_ref().map(|b| ms < b.1).unwrap_or(true) {
                best_mid = Some((file.clone(), ms));
            }
        }
    }

    let choice: Option<String> = if quick { picked.clone().or(best_mid.map(|b| b.0)) } else { None };
    let want_running = was_running || auto;
    let mut running_now = false;

    if let Some(f) = &choice {
        {
            let st = app.state::<AppState>();
            let mut s = st.settings.lock().unwrap();
            s.strategy = f.clone();
            let _ = save_settings(&s);
        }
        if settings.bypass_autostart {
            let _ = register_bypass_task(f);
        }
        if want_running {
            if last_launched.as_ref() == Some(f) && is_running() {
                running_now = true; // the winner is still running, no restart needed
                tune_winws();
            } else {
                let s2 = app.state::<AppState>().settings.lock().unwrap().clone();
                running_now = launch_strategy(&dir, f, &s2).is_ok();
            }
        } else {
            stop_winws();
        }
    } else if want_running {
        let s2 = app.state::<AppState>().settings.lock().unwrap().clone();
        running_now = launch_strategy(&dir, &current, &s2).is_ok();
    } else {
        stop_winws();
    }

    INTENT_BYPASS.store(running_now, Ordering::SeqCst);
    TESTING.store(false, Ordering::SeqCst);
    if auto {
        match &choice {
            Some(f) => notify_if_enabled(&app, "EmzLink", &format!("Выбран режим {}.", pretty_name(f))),
            None => notify_if_enabled(&app, "EmzLink", "Рабочий режим не найден. Проверьте интернет и антивирус."),
        }
    }
    set_status(&app, running_now);
    let _ = app.emit("test-done", TestDone { running: running_now, picked: choice, quick });
}

// ───────────────────────── conflicts with other installs ─────────────────────────

#[derive(Serialize, Clone)]
struct Conflict {
    id: String,
    title: String,
    detail: String,
    folder: Option<String>,
}

fn run_quiet(prog: &str, args: &[&str]) -> bool {
    let mut c = Command::new(prog);
    c.args(args);
    quiet(&mut c);
    c.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Value of `reg query <key> /v <name>` (None if missing).
fn reg_query_value(key: &str, name: &str) -> Option<String> {
    let mut c = Command::new("reg");
    c.args(["query", key, "/v", name]);
    quiet(&mut c);
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    for line in text.lines() {
        let l = line.trim();
        if l.to_lowercase().starts_with(&name.to_lowercase()) {
            for ty in ["REG_EXPAND_SZ", "REG_SZ"] {
                if let Some(i) = l.find(ty) {
                    return Some(l[i + ty.len()..].trim().to_string());
                }
            }
        }
    }
    None
}

fn service_image(name: &str) -> Option<String> {
    reg_query_value(&format!("HKLM\\SYSTEM\\CurrentControlSet\\Services\\{}", name), "ImagePath")
}

/// The exe path from a service command line: "C:\x\winws.exe" --args  /  C:\x\winws.exe --args
fn exe_from_cmdline(cmd: &str) -> PathBuf {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        return PathBuf::from(rest.split('"').next().unwrap_or(""));
    }
    let lower = c.to_lowercase();
    match lower.find(".exe") {
        Some(i) => PathBuf::from(&c[..i + 4]),
        None => PathBuf::from(c),
    }
}

/// X for X\bin\winws.exe when X looks like a zapret-discord-youtube folder.
fn zapret_root_of(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    if !bin.file_name()?.to_string_lossy().eq_ignore_ascii_case("bin") {
        return None;
    }
    let root = bin.parent()?;
    if root.join("service.bat").is_file() && bin.join("winws.exe").is_file() {
        Some(root.to_path_buf())
    } else {
        None
    }
}

fn is_emz_name(n: &str) -> bool {
    n.eq_ignore_ascii_case("emzlink.exe")
}

const SKIP_DIRS: [&str; 12] = [
    "windows", "$recycle.bin", "system volume information", "node_modules", ".git", "target", "appdata",
    "programdata", "$windows.~bt", "recovery", "msocache", "steamlibrary",
];

fn scan_for_emz(dir: &Path, depth: u32, budget: &mut u32, add: &mut dyn FnMut(&Path)) {
    if depth == 0 || *budget == 0 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    add(dir);
    if depth == 1 {
        return;
    }
    for e in rd.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        scan_for_emz(&e.path(), depth - 1, budget, add);
    }
}

/// Other folders (not ours) that contain EmzLink.exe: running copies, autostart entries, and a shallow scan
/// of the user profile, Program Files and the drive roots. Returns (folder, running).
fn other_emz_folders() -> Vec<(PathBuf, bool)> {
    let ours = exe_dir();
    let u = norm_path(&ours);
    let mut found: Vec<(PathBuf, bool)> = Vec::new();
    {
        let mut add = |dir: &Path, running: bool| {
            if !dir.join("EmzLink.exe").is_file() || !has_zapret(dir) {
                return;
            }
            let n = norm_path(dir);
            if n == u || u.starts_with(&format!("{}\\", n)) || n.starts_with(&format!("{}\\", u)) {
                return;
            }
            if let Some(f) = found.iter_mut().find(|f| norm_path(&f.0) == n) {
                f.1 |= running;
            } else {
                found.push((dir.to_path_buf(), running));
            }
        };
        let me = std::process::id();
        for (pid, name) in process_list() {
            if pid != me && is_emz_name(&name) {
                if let Some(d) = image_path(pid).and_then(|p| p.parent().map(|d| d.to_path_buf())) {
                    add(&d, true);
                }
            }
        }
        if let Some(cmd) = reg_query_value(RUN_KEY, "EmzLink") {
            if let Some(d) = exe_from_cmdline(&cmd).parent() {
                add(d, false);
            }
        }
        let mut budget: u32 = 60000;
        let mut roots: Vec<(PathBuf, u32)> = Vec::new();
        if let Some(h) = std::env::var_os("USERPROFILE") {
            roots.push((PathBuf::from(h), 4));
        }
        if let Some(h) = std::env::var_os("LOCALAPPDATA") {
            roots.push((PathBuf::from(h).join("Programs"), 3));
        }
        for v in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(h) = std::env::var_os(v) {
                roots.push((PathBuf::from(h), 2));
            }
        }
        for l in b'C'..=b'Z' {
            roots.push((PathBuf::from(format!("{}:\\", l as char)), 3));
        }
        for (r, depth) in roots {
            scan_for_emz(&r, depth, &mut budget, &mut |d| add(d, false));
        }
    }
    found
}

const RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

fn scan_conflicts_inner() -> Vec<Conflict> {
    let mut v: Vec<Conflict> = Vec::new();

    let foreign = foreign_winws();
    if !foreign.is_empty() {
        let mut paths: Vec<String> = foreign.iter().map(|(_, p)| p.clone()).collect();
        paths.sort();
        paths.dedup();
        let folder = paths
            .first()
            .and_then(|p| zapret_root_of(Path::new(p)))
            .map(|p| p.to_string_lossy().to_string());
        v.push(Conflict {
            id: "winws".into(),
            title: "Уже запущен другой zapret".into(),
            detail: paths.join("\n"),
            folder,
        });
    }

    if let Some(img) = service_image("zapret") {
        let exe = exe_from_cmdline(&img);
        let folder = zapret_root_of(&exe).map(|p| p.to_string_lossy().to_string());
        v.push(Conflict {
            id: "zapret_service".into(),
            title: "Установлена служба zapret (стартует вместе с Windows)".into(),
            detail: exe.to_string_lossy().to_string(),
            folder,
        });
    }

    if !process_ids("goodbyedpi.exe").is_empty() || service_image("GoodbyeDPI").is_some() {
        v.push(Conflict {
            id: "goodbyedpi".into(),
            title: "Найден GoodbyeDPI".into(),
            detail: "Использует тот же драйвер WinDivert и мешает zapret".into(),
            folder: None,
        });
    }

    if !process_ids("ZapretGUI.exe").is_empty() || task_exists("ZapretGUI") {
        v.push(Conflict {
            id: "old_gui".into(),
            title: "Старый ZapretGUI".into(),
            detail: "Запущен или стоит в автозапуске, EmzLink его заменяет".into(),
            folder: None,
        });
    }

    for (dir, running) in other_emz_folders() {
        v.push(Conflict {
            id: "emz_copy".into(),
            title: "Найдена другая копия EmzLink".into(),
            detail: format!("{}{}", dir.display(), if running { " (запущена)" } else { "" }),
            folder: Some(dir.to_string_lossy().to_string()),
        });
    }

    let tg_foreign = tg_split().1;
    let run_cmd = reg_query_value(RUN_KEY, "TgWsProxy");
    let run_foreign = run_cmd
        .as_ref()
        .map(|c| !norm_path(&exe_from_cmdline(c)).eq(&norm_path(&tg_exe())))
        .unwrap_or(false);
    if !tg_foreign.is_empty() || run_foreign {
        let mut detail: Vec<String> = tg_foreign.iter().map(|(_, p)| p.clone()).collect();
        if run_foreign {
            detail.push("есть запись в автозапуске Windows".into());
        }
        detail.dedup();
        v.push(Conflict {
            id: "tg_proxy".into(),
            title: "Уже есть прокси для Telegram (TgWsProxy)".into(),
            detail: detail.join("\n"),
            folder: None,
        });
    }
    v
}

fn user_lines_empty(p: &Path) -> bool {
    !read_lines(p).iter().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#') && t != "domain.example.abc"
    })
}

/// Brings the user's own domain lists and game filter over from an old zapret folder.
fn migrate_user_files(old: &Path, ours: &Path) -> Vec<String> {
    let mut done = Vec::new();
    for name in ["list-general-user.txt", "list-exclude-user.txt", "ipset-exclude-user.txt"] {
        let src = old.join("lists").join(name);
        let dst = ours.join("lists").join(name);
        if src.is_file() && !user_lines_empty(&src) && (!dst.is_file() || user_lines_empty(&dst)) {
            let _ = fs::create_dir_all(ours.join("lists"));
            if fs::copy(&src, &dst).is_ok() {
                done.push(name.to_string());
            }
        }
    }
    let g_src = old.join("utils").join("game_filter.enabled");
    let g_dst = ours.join("utils").join("game_filter.enabled");
    if g_src.is_file() && !g_dst.is_file() {
        let _ = fs::create_dir_all(ours.join("utils"));
        if fs::copy(&g_src, &g_dst).is_ok() {
            done.push("game_filter.enabled".into());
        }
    }
    done
}

/// Only ever deletes something that really is a zapret-discord-youtube folder and is not ours.
fn remove_old_folder(old: &Path, ours: &Path) -> Result<(), String> {
    let o = norm_path(old);
    let u = norm_path(ours);
    if o.len() < 6 || o == u || u.starts_with(&format!("{}\\", o)) || o.starts_with(&format!("{}\\", u)) {
        return Err("папка совпадает с папкой EmzLink".into());
    }
    // never delete a plain folder (Desktop, Downloads...) just because a stray EmzLink.exe lies in it
    if !has_zapret(old) {
        return Err("не похоже на папку zapret или EmzLink".into());
    }
    let mut last = String::new();
    for attempt in 0..6 {
        // anything still running from that folder (winws, tg proxy, a console opened there) locks it
        let me = std::process::id();
        for (pid, _) in process_list() {
            if pid == me {
                continue;
            }
            if let Some(img) = image_path(pid) {
                if norm_path(&img).starts_with(&format!("{}\\", o)) {
                    run_quiet("taskkill", &["/F", "/T", "/PID", &pid.to_string()]);
                }
            }
        }
        if attempt == 2 {
            for drv in ["WinDivert", "WinDivert14"] {
                run_quiet("sc", &["stop", drv]);
            }
        }
        match fs::remove_dir_all(old) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
        thread::sleep(Duration::from_millis(700));
    }
    Err(format!("{} — закройте окна из этой папки и удалите вручную (или после перезагрузки)", last))
}

fn do_resolve(remove_files: bool, migrate: bool, ours: &Path) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    INTENT_BYPASS.store(false, Ordering::SeqCst);
    let found = scan_conflicts_inner();
    let mut folders: Vec<PathBuf> = found.iter().filter_map(|c| c.folder.as_ref().map(PathBuf::from)).collect();
    folders.sort();
    folders.dedup();

    // services first, otherwise the service manager would restart what we kill
    if service_image("zapret").is_some() {
        run_quiet("sc", &["stop", "zapret"]);
        thread::sleep(Duration::from_millis(500));
        if run_quiet("sc", &["delete", "zapret"]) {
            lines.push("Служба zapret удалена".into());
        }
    }
    if service_image("GoodbyeDPI").is_some() {
        run_quiet("sc", &["stop", "GoodbyeDPI"]);
        if run_quiet("sc", &["delete", "GoodbyeDPI"]) {
            lines.push("Служба GoodbyeDPI удалена".into());
        }
    }
    if kill_foreign_winws() > 0 {
        lines.push("Чужой winws.exe остановлен".into());
    }
    if !process_ids("goodbyedpi.exe").is_empty() {
        run_quiet("taskkill", &["/F", "/T", "/IM", "goodbyedpi.exe"]);
        lines.push("GoodbyeDPI остановлен".into());
    }
    if !process_ids("ZapretGUI.exe").is_empty() {
        run_quiet("taskkill", &["/F", "/T", "/IM", "ZapretGUI.exe"]);
    }
    {
        let me = std::process::id();
        let our_exe = std::env::current_exe().unwrap_or_default();
        let mut killed = false;
        for (pid, name) in process_list() {
            if pid != me && is_emz_name(&name) {
                if let Some(p) = image_path(pid) {
                    if !same_path(&p, &our_exe) {
                        run_quiet("taskkill", &["/F", "/T", "/PID", &pid.to_string()]);
                        killed = true;
                    }
                }
            }
        }
        if killed {
            lines.push("Другая копия EmzLink остановлена".into());
        }
        if let Some(cmd) = reg_query_value(RUN_KEY, "EmzLink") {
            if !same_path(&exe_from_cmdline(&cmd), &our_exe) {
                run_quiet("reg", &["delete", RUN_KEY, "/v", "EmzLink", "/f"]);
                lines.push("Автозапуск другой копии EmzLink убран".into());
            }
        }
        // our autostart tasks must point at this copy, not at the one that registered them last
        if task_exists(TASK_APP) {
            let _ = register_app_task();
        }
        if task_exists(TASK_TG) && tg_exe().is_file() {
            let _ = register_tg_task();
        }
    }
    if task_exists("ZapretGUI") {
        delete_task("ZapretGUI");
        lines.push("Автозапуск ZapretGUI убран".into());
    }
    let tg_foreign = tg_split().1;
    for (pid, _) in &tg_foreign {
        run_quiet("taskkill", &["/F", "/T", "/PID", &pid.to_string()]);
    }
    if !tg_foreign.is_empty() {
        lines.push("Другой TgWsProxy остановлен".into());
    }
    if let Some(cmd) = reg_query_value(RUN_KEY, "TgWsProxy") {
        if !norm_path(&exe_from_cmdline(&cmd)).eq(&norm_path(&tg_exe())) {
            run_quiet("reg", &["delete", RUN_KEY, "/v", "TgWsProxy", "/f"]);
            lines.push("Автозапуск другого TgWsProxy убран".into());
        }
    }
    // stale drivers of the other install; winws loads a fresh one when we start
    thread::sleep(Duration::from_millis(400));
    reset_windivert(true);

    for f in folders.iter().filter(|f| !same_path(f, ours)) {
        if migrate {
            let moved = migrate_user_files(f, ours);
            if !moved.is_empty() {
                lines.push(format!("Перенесены ваши списки: {}", moved.join(", ")));
            }
        }
        if remove_files {
            match remove_old_folder(f, ours) {
                Ok(()) => lines.push(format!("Удалена папка {}", f.display())),
                Err(e) => lines.push(format!("Папку {} не удалось удалить: {}", f.display(), e)),
            }
        }
    }
    if lines.is_empty() {
        lines.push("Конфликтов не найдено".into());
    }
    lines
}

#[tauri::command]
async fn scan_conflicts(state: State<'_, AppState>) -> Result<Vec<Conflict>, String> {
    set_our_dir(&zdir(&state));
    Ok(scan_conflicts_inner())
}

#[tauri::command]
async fn resolve_conflicts(
    app: AppHandle,
    state: State<'_, AppState>,
    remove_files: bool,
    migrate: bool,
) -> Result<Vec<String>, String> {
    if TESTING.load(Ordering::SeqCst) {
        return Err("Идёт проверка режимов".into());
    }
    let ours = zdir(&state);
    set_our_dir(&ours);
    let lines = {
        let _l = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        do_resolve(remove_files, migrate, &ours)
    };
    set_status(&app, is_running());
    Ok(lines)
}

// ───────────────────────── commands ─────────────────────────

#[derive(Serialize)]
struct StateDto {
    settings: Settings,
    installed: bool,
    running: bool,
    strategies: Vec<Strategy>,
    version: String,
    catalog: Vec<SvcOut>,
    game_mode: String,
    ipset: String,
    tg: TgDto,
    testing: bool,
    problem: String,
    tweaks_applied: bool,
}

fn local_version(dir: &Path) -> String {
    fs::read_to_string(dir.join(".service").join("version.txt"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn build_state(settings: Settings) -> StateDto {
    let dir = PathBuf::from(&settings.zapret_dir);
    StateDto {
        installed: is_installed(&dir),
        running: is_running(),
        strategies: list_strategies(&dir),
        version: local_version(&dir),
        catalog: SERVICES
            .iter()
            .map(|d| SvcOut { id: d.id, name: d.name, group: d.group, hint: d.domains.first().copied().unwrap_or("") })
            .collect(),
        game_mode: game_mode_get(&dir),
        ipset: ipset_status(&dir),
        tg: tg_dto(),
        testing: TESTING.load(Ordering::SeqCst),
        problem: install_problem(&dir),
        tweaks_applied: tweaks_backup_path().is_file(),
        settings,
    }
}

fn zdir(state: &State<'_, AppState>) -> PathBuf {
    PathBuf::from(state.settings.lock().unwrap().zapret_dir.clone())
}

#[tauri::command]
async fn get_state(state: State<'_, AppState>) -> Result<StateDto, String> {
    let s = {
        let mut s = state.settings.lock().unwrap();
        let resolved = resolve_zapret_dir(&s.zapret_dir, s.zapret_dir_custom)
            .to_string_lossy()
            .to_string();
        if resolved != s.zapret_dir {
            s.zapret_dir = resolved;
            let _ = save_settings(&s);
        }
        set_our_dir(Path::new(&s.zapret_dir));
        s.clone()
    };
    Ok(build_state(s))
}

#[tauri::command]
async fn get_running() -> Result<bool, String> {
    Ok(is_running())
}

#[tauri::command]
async fn start_bypass(
    app: AppHandle,
    state: State<'_, AppState>,
    file: Option<String>,
) -> Result<(), String> {
    if TESTING.load(Ordering::SeqCst) {
        return Err("Идёт проверка режимов".into());
    }
    let (dir, strategy, s) = {
        let mut s = state.settings.lock().unwrap();
        if let Some(f) = file {
            s.strategy = f;
            let _ = save_settings(&s);
        }
        (PathBuf::from(&s.zapret_dir), s.strategy.clone(), s.clone())
    };
    let r = launch_strategy(&dir, &strategy, &s);
    INTENT_BYPASS.store(r.is_ok(), Ordering::SeqCst);
    set_status(&app, is_running());
    r
}

#[tauri::command]
async fn stop_bypass(app: AppHandle) -> Result<(), String> {
    INTENT_BYPASS.store(false, Ordering::SeqCst);
    {
        let _l = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        stop_winws();
    }
    set_status(&app, is_running());
    Ok(())
}

#[tauri::command]
async fn select_strategy(state: State<'_, AppState>, file: String) -> Result<(), String> {
    if !valid_strategy_name(&file) {
        return Err("Некорректное имя режима".into());
    }
    let bypass_autostart = {
        let mut s = state.settings.lock().unwrap();
        s.strategy = file.clone();
        save_settings(&s)?;
        s.bypass_autostart
    };
    if bypass_autostart {
        register_bypass_task(&file)?;
    }
    Ok(())
}

#[tauri::command]
async fn test_strategies(app: AppHandle, state: State<'_, AppState>, quick: Option<bool>) -> Result<(), String> {
    let dir = zdir(&state);
    if !is_installed(&dir) {
        return Err("zapret не найден".into());
    }
    if !curl_available() {
        return Err("Для проверки нужен curl.exe (есть в Windows 10 1803 и новее)".into());
    }
    set_our_dir(&dir);
    if !foreign_winws().is_empty() {
        return Err(CONFLICT_MSG.into());
    }
    if TESTING.swap(true, Ordering::SeqCst) {
        return Err("Проверка уже идёт".into());
    }
    CANCEL_TEST.store(false, Ordering::SeqCst);
    let quick = quick.unwrap_or(false);
    thread::spawn(move || run_tests(app, quick, false));
    Ok(())
}

#[tauri::command]
async fn cancel_test() -> Result<(), String> {
    CANCEL_TEST.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn set_theme(
    state: State<'_, AppState>,
    style: String,
    mode: String,
    accent: String,
) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    s.style = style;
    s.mode = mode;
    s.accent = accent;
    save_settings(&s)
}

fn apply_window_effect(app: &AppHandle, on: bool, dark: bool) -> Result<(), String> {
    use tauri::window::{Effect, EffectsBuilder};
    let Some(w) = app.get_webview_window("main") else { return Ok(()) };
    if !on {
        w.set_effects(None).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let tint = if dark { tauri::window::Color(32, 32, 32, 110) } else { tauri::window::Color(243, 243, 243, 110) };
    let acrylic = EffectsBuilder::new().effect(Effect::Acrylic).color(tint).build();
    if w.set_effects(acrylic).is_err() {
        // older Windows builds: plain blur
        w.set_effects(EffectsBuilder::new().effect(Effect::Blur).color(tint).build())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_transparency(app: AppHandle, state: State<'_, AppState>, enabled: bool, dark: bool, persist: bool) -> Result<(), String> {
    if persist {
        let mut s = state.settings.lock().unwrap();
        s.transparency = enabled;
        save_settings(&s)?;
    }
    apply_window_effect(&app, enabled, dark)
}

#[tauri::command]
async fn set_experimental(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    s.experimental = enabled;
    save_settings(&s)
}

/// Stops every winws.exe that does not belong to this EmzLink folder and waits until they are gone.
fn kill_foreign_winws() -> usize {
    let mut n = 0;
    for _ in 0..4 {
        let list = foreign_winws();
        if list.is_empty() {
            break;
        }
        for (pid, _) in &list {
            run_quiet("taskkill", &["/F", "/T", "/PID", &pid.to_string()]);
            n += 1;
        }
        for _ in 0..20 {
            if foreign_winws().is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    n
}

#[tauri::command]
async fn stop_foreign_winws(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    set_our_dir(&zdir(&state));
    let n = {
        let _l = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        kill_foreign_winws()
    };
    set_status(&app, is_running());
    Ok(n)
}

#[tauri::command]
async fn set_window_bg(app: AppHandle, color: String) -> Result<(), String> {
    let c = color.trim().trim_start_matches('#');
    if c.len() != 6 {
        return Err("bad color".into());
    }
    let n = u32::from_str_radix(c, 16).map_err(|e| e.to_string())?;
    let col = tauri::window::Color(((n >> 16) & 255) as u8, ((n >> 8) & 255) as u8, (n & 255) as u8, 255);
    if let Some(w) = app.get_webview_window("main") {
        w.set_background_color(Some(col)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_service(state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), String> {
    if !SERVICES.iter().any(|d| d.id == id) {
        return Err("Неизвестный сервис".into());
    }
    let mut s = state.settings.lock().unwrap();
    s.services.insert(id, enabled);
    save_settings(&s)
}

#[tauri::command]
async fn add_domain(state: State<'_, AppState>, domain: String) -> Result<Vec<String>, String> {
    let d = clean_domain(&domain).ok_or_else(|| "Неверный адрес сайта".to_string())?;
    let mut s = state.settings.lock().unwrap();
    if !s.custom_domains.contains(&d) {
        s.custom_domains.push(d);
    }
    save_settings(&s)?;
    Ok(s.custom_domains.clone())
}

#[tauri::command]
async fn remove_domain(state: State<'_, AppState>, domain: String) -> Result<Vec<String>, String> {
    let mut s = state.settings.lock().unwrap();
    s.custom_domains.retain(|x| x != &domain);
    save_settings(&s)?;
    Ok(s.custom_domains.clone())
}

#[tauri::command]
async fn set_game_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    game_mode_set(&zdir(&state), &mode)
}

#[tauri::command]
async fn set_ipset_mode(state: State<'_, AppState>, mode: String) -> Result<String, String> {
    let dir = zdir(&state);
    ipset_set(&dir, &mode)?;
    Ok(ipset_status(&dir))
}

#[tauri::command]
async fn update_ipset(state: State<'_, AppState>) -> Result<String, String> {
    let dir = zdir(&state);
    ipset_download(&dir)?;
    Ok(ipset_status(&dir))
}

#[tauri::command]
async fn set_bypass_autostart(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let strategy = state.settings.lock().unwrap().strategy.clone();
    if enabled {
        register_bypass_task(&strategy)?;
    } else {
        delete_task(TASK_BYPASS);
    }
    let mut s = state.settings.lock().unwrap();
    s.bypass_autostart = enabled;
    save_settings(&s)
}

#[tauri::command]
async fn set_app_autostart(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if enabled {
        register_app_task()?;
    } else {
        delete_task(TASK_APP);
    }
    let mut s = state.settings.lock().unwrap();
    s.app_autostart = enabled;
    save_settings(&s)
}

#[tauri::command]
async fn tg_state() -> Result<TgDto, String> {
    Ok(tg_dto())
}

#[tauri::command]
async fn tg_toggle(state: State<'_, AppState>, enabled: bool) -> Result<TgDto, String> {
    if enabled {
        if !tg_split().1.is_empty() {
            return Err("CONFLICT: уже запущен другой TgWsProxy. Откройте проверку совместимости.".into());
        }
        if !tg_exe().is_file() {
            tg_install()?;
        }
        tg_start()?;
        INTENT_TG.store(true, Ordering::SeqCst);
    } else {
        INTENT_TG.store(false, Ordering::SeqCst);
        tg_stop();
        delete_task(TASK_TG);
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.tg_proxy = enabled;
        if !enabled {
            s.tg_autostart = false;
        }
        save_settings(&s)?;
    }
    Ok(tg_dto())
}

#[tauri::command]
async fn tg_set_autostart(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if enabled {
        if !tg_exe().is_file() {
            return Err("Сначала включите прокси".into());
        }
        register_tg_task()?;
    } else {
        delete_task(TASK_TG);
    }
    let mut s = state.settings.lock().unwrap();
    s.tg_autostart = enabled;
    save_settings(&s)
}

#[tauri::command]
async fn choose_dir(state: State<'_, AppState>) -> Result<Option<StateDto>, String> {
    // an invisible always-on-top owner window keeps the dialog in front of EmzLink
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$f = New-Object System.Windows.Forms.Form
$f.TopMost = $true; $f.ShowInTaskbar = $false; $f.Opacity = 0
$f.StartPosition = 'CenterScreen'; $f.Size = New-Object System.Drawing.Size(1,1)
$f.Show()
$d = New-Object System.Windows.Forms.FolderBrowserDialog
$d.Description = 'Папка с zapret'
if ($d.ShowDialog($f) -eq 'OK') { Write-Output $d.SelectedPath }
$f.Close()
"#;
    let picked = powershell_enc_opts(script, &[], true)?;
    if picked.is_empty() {
        return Ok(None);
    }
    let s = {
        let mut s = state.settings.lock().unwrap();
        let target = find_zapret_in(Path::new(&picked)).unwrap_or_else(|| PathBuf::from(&picked));
        s.zapret_dir = target.to_string_lossy().to_string();
        s.zapret_dir_custom = true;
        set_our_dir(&target);
        save_settings(&s)?;
        s.clone()
    };
    Ok(Some(build_state(s)))
}

#[tauri::command]
async fn open_dir(state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.settings.lock().unwrap().zapret_dir.clone();
    let _ = fs::create_dir_all(&dir);
    Command::new("explorer.exe")
        .arg(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct UpdateInfo {
    local: String,
    remote: String,
    has_update: bool,
}

#[tauri::command]
async fn check_updates(state: State<'_, AppState>) -> Result<UpdateInfo, String> {
    let dir = zdir(&state);
    let local = local_version(&dir);
    let script = format!(
        "(Invoke-WebRequest -Uri '{}' -Headers @{{'Cache-Control'='no-cache'}} -UseBasicParsing -TimeoutSec 10).Content.Trim()",
        LATEST_VERSION_URL
    );
    let remote = powershell(&script, &[], false)
        .map_err(|_| "Не удалось проверить обновления. Проверьте интернет.".to_string())?;
    let has_update = !remote.is_empty() && remote != local;
    Ok(UpdateInfo {
        local,
        remote,
        has_update,
    })
}

/// Downloads the latest zapret-discord-youtube archive and copies it into the zapret folder.
/// User lists (lists/*-user.txt) and EmzLink files are never overwritten.
#[tauri::command]
async fn update_zapret(app: AppHandle, state: State<'_, AppState>) -> Result<StateDto, String> {
    let dir = zdir(&state);
    let remote = powershell(
        &format!(
            "(Invoke-WebRequest -Uri '{}' -UseBasicParsing -TimeoutSec 15).Content.Trim()",
            LATEST_VERSION_URL
        ),
        &[],
        false,
    )
    .map_err(|_| "Не удалось получить версию с GitHub. Проверьте интернет.".to_string())?;
    if remote.is_empty()
        || remote
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'))
    {
        return Err("GitHub вернул неожиданный ответ".into());
    }

    if TESTING.load(Ordering::SeqCst) {
        return Err("Идёт проверка режимов".into());
    }
    let was_running = is_running();
    INTENT_BYPASS.store(false, Ordering::SeqCst);
    {
        let _l = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        stop_winws();
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let url = format!("{}{}.zip", ARCHIVE_URL, remote);
    let script = r#"
$ErrorActionPreference='Stop'
[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
$tmp = Join-Path $env:TEMP 'emzlink_update'
if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force }
New-Item -ItemType Directory -Path $tmp | Out-Null
$zip = Join-Path $tmp 'z.zip'
Invoke-WebRequest -Uri $env:EMZ_URL -OutFile $zip -UseBasicParsing
Expand-Archive -Path $zip -DestinationPath (Join-Path $tmp 'x') -Force
$src = (Get-ChildItem (Join-Path $tmp 'x') -Directory | Select-Object -First 1).FullName
$dst = $env:EMZ_DIR
Get-ChildItem $src -Recurse -File | ForEach-Object {
  $rel = $_.FullName.Substring($src.Length + 1)
  if ($rel -like 'lists\*-user.txt') { return }
  if ($rel -like 'lists\ipset-all.txt*') { return }
  if ($rel -like '.git*' -or $rel -like '.github\*') { return }
  if ($rel -notlike '*\*' -and $rel -like '*.exe') { return }
  $target = Join-Path $dst $rel
  New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
  Copy-Item $_.FullName $target -Force
}
Remove-Item $tmp -Recurse -Force
"#;
    powershell(
        script,
        &[("EMZ_URL", &url), ("EMZ_DIR", &dir.to_string_lossy())],
        false,
    )
    .map_err(|e| format!("Ошибка загрузки: {}", e))?;

    let s = state.settings.lock().unwrap().clone();
    if was_running {
        let r = launch_strategy(&dir, &s.strategy, &s);
        INTENT_BYPASS.store(r.is_ok(), Ordering::SeqCst);
    }
    set_status(&app, is_running());
    Ok(build_state(s))
}

#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("tg://")) {
        return Err("bad url".into());
    }
    let mut c = Command::new("rundll32");
    c.args(["url.dll,FileProtocolHandler", &url]);
    quiet(&mut c);
    c.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn window_minimize(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn window_toggle_max(app: AppHandle) -> Result<bool, String> {
    let w = app.get_webview_window("main").ok_or("no window")?;
    if w.is_maximized().unwrap_or(false) {
        w.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        w.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[tauri::command]
async fn window_hide(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn quit_app(app: AppHandle) -> Result<(), String> {
    quit(&app);
    Ok(())
}

#[tauri::command]
async fn ui_ready(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    UI_READY.store(true, Ordering::SeqCst);
    if !state.start_hidden {
        show_main(&app);
    }
    Ok(())
}

// ───────────────────────── notifications ─────────────────────────

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for ch in data.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if ch.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if ch.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Like `powershell`, but the script travels as -EncodedCommand (no quoting problems at all).
/// PowerShell prints errors as CLIXML ("#< CLIXML <Objs ...>") when its streams are redirected:
/// turn that into a short readable message.
fn clean_ps_error(raw: &str) -> String {
    let t = raw.trim();
    if !t.contains("CLIXML") {
        return t.to_string();
    }
    let mut msgs: Vec<String> = Vec::new();
    let mut rest = t;
    while let Some(i) = rest.find("<S S=\"Error\">") {
        let after = &rest[i + "<S S=\"Error\">".len()..];
        let end = after.find("</S>").unwrap_or(after.len());
        let piece = after[..end]
            .replace("_x000D__x000A_", "\n")
            .replace("_x000D_", "")
            .replace("_x000A_", "\n")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&amp;", "&");
        for line in piece.lines() {
            let l = line.trim();
            if !l.is_empty() && !l.starts_with('+') && !l.starts_with("~") && !l.starts_with("CategoryInfo") && !l.starts_with("FullyQualifiedErrorId") {
                msgs.push(l.to_string());
            }
        }
        rest = &after[end..];
        if msgs.len() >= 2 {
            break;
        }
    }
    if msgs.is_empty() {
        "Ошибка PowerShell".into()
    } else {
        msgs.truncate(2);
        msgs.join(" ")
    }
}

fn powershell_enc(script: &str, envs: &[(&str, &str)]) -> Result<String, String> {
    powershell_enc_opts(script, envs, false)
}

fn powershell_enc_opts(script: &str, envs: &[(&str, &str)], sta: bool) -> Result<String, String> {
    let full = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ProgressPreference='SilentlyContinue'; try {{ [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12 }} catch {{}}; {}",
        script
    );
    let utf16: Vec<u8> = full.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-OutputFormat", "Text"]);
    if sta {
        c.arg("-STA");
    }
    c.args(["-EncodedCommand", &b64(&utf16)]);
    for (k, v) in envs {
        c.env(k, v);
    }
    quiet(&mut c);
    let out = c.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(clean_ps_error(&if err.is_empty() { stdout } else { err }))
    }
}

fn app_data_dir() -> PathBuf {
    settings_path().parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir)
}

fn tweaks_backup_path() -> PathBuf {
    app_data_dir().join("tweaks_backup.json")
}

fn notify_icon() -> Option<PathBuf> {
    let p = app_data_dir().join("icon.png");
    if !p.is_file() {
        fs::create_dir_all(app_data_dir()).ok()?;
        fs::write(&p, include_bytes!("../icons/128x128.png")).ok()?;
    }
    Some(p)
}

/// Windows toast under the name "EmzLink" (the app id is registered under HKCU, no installer needed).
fn toast(title: &str, body: &str) {
    let icon = notify_icon().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let script = r#"
$id='EmzLink.App'
$rk="HKCU:\Software\Classes\AppUserModelId\$id"
if(-not (Test-Path -LiteralPath $rk)){ New-Item -Path $rk -Force | Out-Null }
New-ItemProperty -Path $rk -Name DisplayName -Value 'EmzLink' -PropertyType String -Force | Out-Null
if($env:EMZ_ICON){ New-ItemProperty -Path $rk -Name IconUri -Value $env:EMZ_ICON -PropertyType String -Force | Out-Null }
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType=WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType=WindowsRuntime] | Out-Null
$t=[System.Security.SecurityElement]::Escape($env:EMZ_TITLE)
$b=[System.Security.SecurityElement]::Escape($env:EMZ_BODY)
$xml=New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml("<toast><visual><binding template='ToastGeneric'><text>$t</text><text>$b</text></binding></visual></toast>")
$toast=New-Object Windows.UI.Notifications.ToastNotification($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($id).Show($toast)
"#;
    let _ = powershell_enc(script, &[("EMZ_TITLE", title), ("EMZ_BODY", body), ("EMZ_ICON", &icon)]);
}

fn notify_if_enabled(app: &AppHandle, title: &str, body: &str) {
    let on = app
        .try_state::<AppState>()
        .map(|s| s.settings.lock().unwrap().notify)
        .unwrap_or(true);
    if on {
        let (t, b) = (title.to_string(), body.to_string());
        thread::spawn(move || toast(&t, &b));
    }
}

/// Checks every few seconds that things the user switched on are still alive.
fn spawn_watchdog(app: AppHandle) {
    thread::spawn(move || {
        let (mut miss_b, mut miss_t) = (0u32, 0u32);
        let (mut heal_tick, mut heal_fail) = (0u32, 0u32);
        let mut last_heal = std::time::Instant::now() - Duration::from_secs(3600);
        loop {
            thread::sleep(Duration::from_secs(4));
            if TESTING.load(Ordering::SeqCst) || LAUNCHING.load(Ordering::SeqCst) {
                miss_b = 0;
                continue;
            }
            if INTENT_BYPASS.load(Ordering::SeqCst) {
                if is_running() {
                    miss_b = 0;
                } else {
                    miss_b += 1;
                    if miss_b >= 2 {
                        miss_b = 0;
                        INTENT_BYPASS.store(false, Ordering::SeqCst);
                        set_status(&app, false);
                        let _ = app.emit("bypass-dropped", "bypass");
                        notify_if_enabled(&app, "EmzLink", "Обход отключился. Откройте приложение и включите его снова.");
                    }
                }
            } else {
                miss_b = 0;
                // started by the autostart task (or anyone else) while the app was already open
                if is_running() && LAUNCH_LOCK.try_lock().is_ok() {
                    INTENT_BYPASS.store(true, Ordering::SeqCst);
                }
            }
            // auto-pick when sites stop opening although the bypass process is alive
            heal_tick += 1;
            if heal_tick >= 15 {
                heal_tick = 0;
                let settings = app.state::<AppState>().settings.lock().unwrap().clone();
                if settings.auto_heal
                    && INTENT_BYPASS.load(Ordering::SeqCst)
                    && is_running()
                    && !TESTING.load(Ordering::SeqCst)
                    && !LAUNCHING.load(Ordering::SeqCst)
                    && curl_available()
                    && last_heal.elapsed() > Duration::from_secs(600)
                {
                    let (net_ok, _) = probe(&["http://www.msftconnecttest.com/connecttest.txt"]);
                    if net_ok == 1 {
                        let (ok, _) = probe(&test_urls(&settings));
                        heal_fail = if ok == 0 { heal_fail + 1 } else { 0 };
                        if heal_fail >= 2 {
                            heal_fail = 0;
                            last_heal = std::time::Instant::now();
                            if !TESTING.swap(true, Ordering::SeqCst) {
                                CANCEL_TEST.store(false, Ordering::SeqCst);
                                notify_if_enabled(&app, "EmzLink", "Сайты перестали открываться. Подбираем другой режим…");
                                let a = app.clone();
                                thread::spawn(move || run_tests(a, true, true));
                            }
                        }
                    }
                } else {
                    heal_fail = 0;
                }
            }
            if INTENT_TG.load(Ordering::SeqCst) && tg_exe().is_file() {
                if tg_running() {
                    miss_t = 0;
                } else {
                    miss_t += 1;
                    if miss_t >= 2 {
                        miss_t = 0;
                        // one quiet restart attempt: Telegram reconnects to the same address by itself
                        if tg_start().is_err() {
                            INTENT_TG.store(false, Ordering::SeqCst);
                            let _ = app.emit("bypass-dropped", "tg");
                            notify_if_enabled(&app, "EmzLink", "Прокси для Telegram остановился.");
                        }
                    }
                }
            } else {
                miss_t = 0;
            }
        }
    });
}

/// "Reset to factory settings": removes every autostart task, stops Telegram proxy, restores network
/// tweaks, resets game filter / IPSet and writes default settings (the zapret folder is kept).
#[tauri::command]
async fn reset_all(app: AppHandle, state: State<'_, AppState>) -> Result<StateDto, String> {
    delete_task(TASK_APP);
    delete_task(TASK_BYPASS);
    delete_task(TASK_TG);
    INTENT_TG.store(false, Ordering::SeqCst);
    tg_stop();
    if tweaks_backup_path().is_file() {
        let _ = run_tweaks("reset", "{}");
    }
    let dir = zdir(&state);
    let _ = game_mode_set(&dir, "off");
    let _ = ipset_set(&dir, "none");
    let _ = prepare_lists(&dir, &Settings::default());
    let fresh = {
        let mut s = state.settings.lock().unwrap();
        let mut d = Settings::default();
        d.zapret_dir = s.zapret_dir.clone();
        d.zapret_dir_custom = s.zapret_dir_custom;
        *s = d;
        save_settings(&s)?;
        s.clone()
    };
    set_status(&app, is_running());
    Ok(build_state(fresh))
}

#[tauri::command]
async fn set_auto_heal(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    s.auto_heal = enabled;
    save_settings(&s)
}

#[tauri::command]
async fn set_notify(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut s = state.settings.lock().unwrap();
        s.notify = enabled;
        save_settings(&s)?;
    }
    if enabled {
        thread::spawn(|| toast("EmzLink", "Уведомления включены."));
    }
    Ok(())
}

// ───────────────────────── network tweaks (real apply + rollback) ─────────────────────────

const TWEAK_SCRIPT: &str = r#"
$ErrorActionPreference='Stop'
$mode=$env:EMZ_MODE; $bp=$env:EMZ_BACKUP
$ifRoot='HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces'
$ifReg='HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces'
$mmPs='HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile'
$mmReg='HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile'
$dvrPs='HKCU:\System\GameConfigStore'; $dvrReg='HKCU\System\GameConfigStore'
$capPs='HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR'; $capReg='HKCU\Software\Microsoft\Windows\CurrentVersion\GameDVR'
$doPs='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config'; $doReg='HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config'
$warn=New-Object System.Collections.ArrayList

function GetVal($p,$n){ try { (Get-ItemProperty -LiteralPath $p -Name $n -ErrorAction Stop).$n } catch { $null } }
function RegSet($key,$name,$val){ try { $u=[int64]$val; if($u -lt 0){ $u+=4294967296 }; & reg.exe add $key /v $name /t REG_DWORD /d $u /f | Out-Null } catch { [void]$warn.Add("reg $name") } }
function RegDel($key,$name){ try { & reg.exe delete $key /v $name /f 2>$null | Out-Null } catch { } }
function RegRestore($key,$name,$val){ if($null -eq $val){ RegDel $key $name } else { RegSet $key $name $val } }
function Step($name,[scriptblock]$sb){ try { & $sb } catch { [void]$warn.Add("$name") } }

$ads=@(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'Up' })

function MakeBackup(){
  $b=[ordered]@{ tcp=[ordered]@{}; mtu=[ordered]@{}; dns=[ordered]@{}; nic=[ordered]@{}; throttle=$null; tcpset=$null; power=$null; dvr=$null; dopt=$null }
  foreach($a in $ads){
    $g=[string]$a.InterfaceGuid; $k="$ifRoot\$g"
    $b.tcp[$g]=[ordered]@{ ack=(GetVal $k 'TcpAckFrequency'); nodelay=(GetVal $k 'TCPNoDelay'); delack=(GetVal $k 'TcpDelAckTicks') }
    $b.dns[$g]=[ordered]@{ name=$a.Name; ns=(GetVal $k 'NameServer') }
    $ip=Get-NetIPInterface -InterfaceIndex $a.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue
    if($ip){ $b.mtu[$g]=[ordered]@{ name=$a.Name; mtu=[int]$ip.NlMtu } }
    Step "nic backup" { $pm=Get-NetAdapterPowerManagement -Name $a.Name -ErrorAction Stop; $b.nic[$g]=[ordered]@{ name=$a.Name; allow=[string]$pm.AllowComputerToTurnOffDevice } }
  }
  $b.throttle=[ordered]@{ nti=(GetVal $mmPs 'NetworkThrottlingIndex'); sr=(GetVal $mmPs 'SystemResponsiveness') }
  Step "tcp settings backup" { $t=Get-NetTCPSetting -SettingName InternetCustom -ErrorAction Stop; $b.tcpset=[ordered]@{ ecn=[string]$t.EcnCapability; ts=[string]$t.Timestamps } }
  Step "power backup" { $o=(powercfg /getactivescheme | Out-String); $m=[regex]::Match($o,'[0-9a-fA-F]{8}-([0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}'); if($m.Success){ $b.power=$m.Value } }
  $b.dvr=[ordered]@{ dvr=(GetVal $dvrPs 'GameDVR_Enabled'); cap=(GetVal $capPs 'AppCaptureEnabled') }
  $b.dopt=[ordered]@{ mode=(GetVal $doPs 'DODownloadMode') }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $bp) | Out-Null
  $b | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $bp -Encoding UTF8
}

function Prop($o,$n){ if($null -eq $o){ return $null }; return $o.PSObject.Properties[$n] }

function RestoreAll(){
  if(-not (Test-Path -LiteralPath $bp)){ return $false }
  $b=Get-Content -LiteralPath $bp -Raw -Encoding UTF8 | ConvertFrom-Json
  if($b.tcp){ foreach($p in $b.tcp.PSObject.Properties){
    if(Test-Path -LiteralPath "$ifRoot\$($p.Name)"){
      $k="$ifReg\$($p.Name)"; $v=$p.Value
      RegRestore $k 'TcpAckFrequency' $v.ack; RegRestore $k 'TCPNoDelay' $v.nodelay; RegRestore $k 'TcpDelAckTicks' $v.delack
    }
  } }
  if($b.throttle){ RegRestore $mmReg 'NetworkThrottlingIndex' $b.throttle.nti; RegRestore $mmReg 'SystemResponsiveness' $b.throttle.sr }
  if($b.dvr){ RegRestore $dvrReg 'GameDVR_Enabled' $b.dvr.dvr; RegRestore $capReg 'AppCaptureEnabled' $b.dvr.cap }
  if($b.dopt){ RegRestore $doReg 'DODownloadMode' $b.dopt.mode }
  if($b.tcpset){ Step "tcp settings" { Set-NetTCPSetting -SettingName InternetCustom -EcnCapability $b.tcpset.ecn -Timestamps $b.tcpset.ts } }
  if($b.power){ Step "power plan" { & powercfg /setactive $b.power | Out-Null } }
  foreach($a in @(Get-NetAdapter -Physical -ErrorAction SilentlyContinue)){
    $g=[string]$a.InterfaceGuid
    $m=Prop $b.mtu $g
    if($m){ Step "mtu $($a.Name)" { Set-NetIPInterface -InterfaceIndex $a.ifIndex -AddressFamily IPv4 -NlMtuBytes ([int]$m.Value.mtu) } }
    $d=Prop $b.dns $g
    if($d){
      Step "dns $($a.Name)" {
        $ns=[string]$d.Value.ns
        if([string]::IsNullOrWhiteSpace($ns)){ Set-DnsClientServerAddress -InterfaceIndex $a.ifIndex -ResetServerAddresses }
        else { Set-DnsClientServerAddress -InterfaceIndex $a.ifIndex -ServerAddresses @($ns -split '[,\s]+' | Where-Object { $_ }) }
      }
    }
    $n=Prop $b.nic $g
    if($n -and ($n.Value.allow -eq 'Enabled' -or $n.Value.allow -eq 'Disabled')){
      Step "nic power $($a.Name)" { Set-NetAdapterPowerManagement -Name $a.Name -AllowComputerToTurnOffDevice $n.Value.allow }
    }
  }
  return $true
}

if($mode -eq 'apply'){
  $cfg=$env:EMZ_CFG | ConvertFrom-Json
  if(-not (Test-Path -LiteralPath $bp)){ MakeBackup } else { [void](RestoreAll) }

  if($cfg.tcp){ foreach($a in $ads){ $k="$ifReg\$([string]$a.InterfaceGuid)"; RegSet $k 'TcpAckFrequency' 1; RegSet $k 'TCPNoDelay' 1; RegSet $k 'TcpDelAckTicks' 0 } }
  if($cfg.throttle){ RegSet $mmReg 'NetworkThrottlingIndex' 4294967295; RegSet $mmReg 'SystemResponsiveness' 0 }
  if($cfg.tcpset){ Step "tcp settings" { Set-NetTCPSetting -SettingName InternetCustom -EcnCapability Disabled -Timestamps Disabled } }
  if($cfg.power){ Step "power plan" { & powercfg /setactive SCHEME_MIN | Out-Null } }
  if($cfg.nicpower){ foreach($a in $ads){ Step "nic power $($a.Name)" { Set-NetAdapterPowerManagement -Name $a.Name -AllowComputerToTurnOffDevice Disabled } } }
  if($cfg.gamedvr){ RegSet $dvrReg 'GameDVR_Enabled' 0; RegSet $capReg 'AppCaptureEnabled' 0 }
  if($cfg.dopt){ RegSet $doReg 'DODownloadMode' 0 }
  if($cfg.mtu -ne 'default'){ $n=[int]$cfg.mtu; foreach($a in $ads){ Step "mtu $($a.Name)" { Set-NetIPInterface -InterfaceIndex $a.ifIndex -AddressFamily IPv4 -NlMtuBytes $n } } }
  $servers=$null
  switch($cfg.dns){
    'cloudflare' { $servers=@('1.1.1.1','1.0.0.1') }
    'google'     { $servers=@('8.8.8.8','8.8.4.4') }
    'quad9'      { $servers=@('9.9.9.9','149.112.112.112') }
    'opendns'    { $servers=@('208.67.222.222','208.67.220.220') }
  }
  foreach($a in $ads){
    Step "dns $($a.Name)" {
      if($servers){ Set-DnsClientServerAddress -InterfaceIndex $a.ifIndex -ServerAddresses $servers }
      elseif($cfg.dns -eq 'dhcp'){ Set-DnsClientServerAddress -InterfaceIndex $a.ifIndex -ResetServerAddresses }
    }
  }
  # read the values back: report what Windows did not accept
  function Chk($ps,$n,$exp){ $v=GetVal $ps $n; if($null -ne $v -and $v -lt 0){ $v=[int64]$v+4294967296 }; if($null -eq $v -or [int64]$v -ne [int64]$exp){ [void]$warn.Add("check $n") } }
  if($cfg.tcp){ foreach($a in $ads){ Chk "$ifRoot\$([string]$a.InterfaceGuid)" 'TcpAckFrequency' 1 } }
  if($cfg.throttle){ Chk $mmPs 'NetworkThrottlingIndex' 4294967295; Chk $mmPs 'SystemResponsiveness' 0 }
  if($cfg.gamedvr){ Chk $dvrPs 'GameDVR_Enabled' 0 }
  if($cfg.dopt){ Chk $doPs 'DODownloadMode' 0 }
  if($cfg.mtu -ne 'default'){ foreach($a in $ads){ $ip=Get-NetIPInterface -InterfaceIndex $a.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue; if($ip -and [int]$ip.NlMtu -ne [int]$cfg.mtu){ [void]$warn.Add("check mtu $($a.Name)") } } }
}
elseif($mode -eq 'reset'){
  if(RestoreAll){ Remove-Item -LiteralPath $bp -Force -ErrorAction SilentlyContinue }
}
try { Clear-DnsClientCache } catch { }
foreach($w in $warn){ Write-Output "WARN:$w" }
Write-Output 'OK'
"#;

fn run_tweaks(mode: &str, cfg_json: &str) -> Result<String, String> {
    let bp = tweaks_backup_path();
    let out = powershell_enc(
        TWEAK_SCRIPT,
        &[("EMZ_MODE", mode), ("EMZ_CFG", cfg_json), ("EMZ_BACKUP", &bp.to_string_lossy())],
    )?;
    if !out.lines().any(|l| l.trim() == "OK") {
        return Err("Не удалось применить настройки сети. Запустите EmzLink от имени администратора и повторите.".into());
    }
    let warns = out.lines().filter(|l| l.starts_with("WARN:")).count();
    Ok(if warns == 0 { "OK".into() } else { format!("OK ({} предупрежд.)", warns) })
}

#[tauri::command]
async fn tweaks_apply(state: State<'_, AppState>, cfg: TweakCfg) -> Result<String, String> {
    if !["default", "1500", "1492"].contains(&cfg.mtu.as_str())
        || !["dhcp", "cloudflare", "google", "quad9", "opendns"].contains(&cfg.dns.as_str())
    {
        return Err("Неверные параметры".into());
    }
    let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    let r = run_tweaks("apply", &json)?;
    let mut s = state.settings.lock().unwrap();
    s.tweaks = cfg;
    save_settings(&s)?;
    Ok(r)
}

#[tauri::command]
async fn flush_dns() -> Result<(), String> {
    if run_quiet("ipconfig", &["/flushdns"]) { Ok(()) } else { Err("Не удалось очистить кэш DNS".into()) }
}

#[tauri::command]
async fn tweaks_reset() -> Result<String, String> {
    run_tweaks("reset", "{}")
}

// ───────────────────────── app glue ─────────────────────────

fn set_status(app: &AppHandle, running: bool) {
    let _ = app.emit("status", running);
    if let Some(t) = app.tray_by_id("main") {
        let _ = t.set_tooltip(Some(if running { "EmzLink — обход включён" } else { "EmzLink — выключено" }));
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn quit(app: &AppHandle) {
    let (keep_bypass, keep_tg) = app
        .try_state::<AppState>()
        .map(|s| {
            let s = s.settings.lock().unwrap();
            (s.bypass_autostart, s.tg_autostart)
        })
        .unwrap_or((false, false));
    if !keep_bypass {
        stop_winws();
    }
    if !keep_tg {
        tg_stop();
    }
    app.exit(0);
}

fn toggle_from_tray(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        if TESTING.load(Ordering::SeqCst) {
            return; // a mode check is running: ignore the tray click
        }
        if is_running() {
            INTENT_BYPASS.store(false, Ordering::SeqCst);
            let _l = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            stop_winws();
        } else if let Some(st) = app.try_state::<AppState>() {
            let s = st.settings.lock().unwrap().clone();
            let r = launch_strategy(&PathBuf::from(&s.zapret_dir), &s.strategy, &s);
            INTENT_BYPASS.store(r.is_ok(), Ordering::SeqCst);
        }
        set_status(&app, is_running());
    });
}

/// Headless mode used by the "bypass autostart" scheduled task:
/// `EmzLink.exe --run-strategy "general (ALT).bat"`
fn headless_run(file: &str) {
    let s = load_settings();
    // the app (or another task run) may have started it already: never start a second copy
    let _lock = ProcLock::acquire();
    if is_running() {
        return;
    }
    drop(_lock);
    let _ = launch_strategy(&PathBuf::from(&s.zapret_dir), file, &s);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--run-strategy") {
        if let Some(file) = args.get(i + 1) {
            headless_run(file);
        }
        return;
    }
    let start_hidden = args.iter().any(|a| a == "--minimized");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .manage(AppState {
            settings: Mutex::new(load_settings()),
            start_hidden,
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_running,
            start_bypass,
            stop_bypass,
            select_strategy,
            test_strategies,
            cancel_test,
            set_theme,
            set_window_bg,
            set_transparency,
            set_experimental,
            stop_foreign_winws,
            set_service,
            add_domain,
            remove_domain,
            set_game_mode,
            set_ipset_mode,
            update_ipset,
            set_bypass_autostart,
            set_app_autostart,
            tg_state,
            tg_toggle,
            tg_set_autostart,
            choose_dir,
            open_dir,
            check_updates,
            update_zapret,
            open_url,
            window_minimize,
            window_hide,
            window_toggle_max,
            quit_app,
            set_notify,
            set_auto_heal,
            reset_all,
            scan_conflicts,
            resolve_conflicts,
            tweaks_apply,
            tweaks_reset,
            flush_dns,
            ui_ready
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // keep autostart flags honest if the user removed the tasks manually
            if let Some(st) = app.try_state::<AppState>() {
                let mut s = st.settings.lock().unwrap();
                let (a, b, c) = (task_exists(TASK_APP), task_exists(TASK_BYPASS), task_exists(TASK_TG));
                if s.app_autostart != a || s.bypass_autostart != b || s.tg_autostart != c {
                    s.app_autostart = a;
                    s.bypass_autostart = b;
                    s.tg_autostart = c;
                    let _ = save_settings(&s);
                }
                if s.tg_proxy && !tg_exe().is_file() {
                    // fresh install (the tgproxy folder is removed on uninstall): the proxy stays OFF until the user turns it on
                    s.tg_proxy = false;
                    s.tg_autostart = false;
                    delete_task(TASK_TG);
                    let _ = save_settings(&s);
                }
                if s.tg_proxy {
                    INTENT_TG.store(true, Ordering::SeqCst);
                    thread::spawn(|| {
                        let _ = tg_start();
                    });
                }
            }
            if is_running() {
                INTENT_BYPASS.store(true, Ordering::SeqCst);
            }
            spawn_watchdog(app.handle().clone());

            // autostart tasks keep the old exe path after the folder was moved/renamed: rewrite them
            if let Some(st) = app.try_state::<AppState>() {
                let strategy = st.settings.lock().unwrap().strategy.clone();
                thread::spawn(move || {
                    if task_exists(TASK_APP) {
                        let _ = register_app_task();
                    }
                    if task_exists(TASK_BYPASS) {
                        let _ = register_bypass_task(&strategy);
                    }
                    if task_exists(TASK_TG) && tg_exe().is_file() {
                        let _ = register_tg_task();
                    }
                });
            }

            // safety net: if the page never reports ready (script error), still show the window
            {
                let handle = app.handle().clone();
                let hidden_start = app.try_state::<AppState>().map(|s| s.start_hidden).unwrap_or(false);
                if !hidden_start {
                    thread::spawn(move || {
                        thread::sleep(Duration::from_secs(8));
                        if !UI_READY.load(Ordering::SeqCst) {
                            show_main(&handle);
                        }
                    });
                }
            }

            let open = MenuItem::with_id(app, "open", "Открыть EmzLink", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Включить / выключить", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &exit])?;

            let mut tray = TrayIconBuilder::with_id("main")
                .tooltip("EmzLink")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "toggle" => toggle_from_tray(app),
                    "quit" => quit(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let tray_icon = tray.build(app)?;
            let _ = tray_icon.set_tooltip(Some(if is_running() { "EmzLink — обход включён" } else { "EmzLink — выключено" }));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running EmzLink");
}
