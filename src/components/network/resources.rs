//! network — what this machine can lend: CPU, memory, GPU, disk, Google Drive, internet, power and
//! idle time, sent with every heartbeat so the Machines page and the pool know who can do what.
//!
//! Every reading goes through [`Probe`], so the parsers are tested on captured command output and
//! nothing here needs a real Mac or PC. Each command has a timeout; a failed read is `None` and the
//! field is simply left out (kun treats unknown as unknown, never as zero).

use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{LazyLock, Mutex, PoisonError, mpsc},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// A small public file, fetched with a range so it is always about 2 MB, for the speed reading.
pub const SPEED_URL: &str = "https://cdn.databayt.org/hogwarts/story.mp4";
const SPEED_BYTES: u64 = 2 * 1024 * 1024;
/// The speed is measured at most this often.
pub const SPEED_EVERY_SECS: u64 = 3600;
/// How long a Drive size is trusted before `du` runs again.
const DRIVE_TTL_SECS: u64 = 3600;
const QUICK: Duration = Duration::from_secs(3);
const SLOW: Duration = Duration::from_secs(8);
const DU_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Cpu {
    pub model: String,
    pub cores: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Gpu {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cores: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vram_bytes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Drive {
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub used_bytes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Internet {
    pub online: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub latency_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mbps_down: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub measured_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Power {
    pub on_ac: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub battery_percent: Option<f64>,
}

/// What a machine can lend. Every field is optional, as in kun's `resourcesSchema`. Bytes are bytes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "network/types.ts")]
pub struct Resources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cpu: Option<Cpu>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ram_total: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ram_free: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gpu: Option<Vec<Gpu>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub disk_free: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub drive: Option<Drive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub internet: Option<Internet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub power: Option<Power>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub idle_secs: Option<f64>,
}

#[allow(clippy::cast_precision_loss)] // sizes far below 2^53
fn f(n: u64) -> f64 {
    n as f64
}

// ---- the probe -----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Other,
}

/// How this module touches the machine: run a command, list the Drive folders.
pub trait Probe {
    fn os(&self) -> Os;
    /// stdout of the command, or `None` when it is missing, fails, or outlives `timeout`.
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Option<String>;
    /// `~/Library/CloudStorage/GoogleDrive-*` (macOS).
    fn drive_dirs(&self) -> Vec<PathBuf>;
    /// The Drive app is installed even when nobody signed in yet.
    fn drive_app(&self) -> bool;
}

pub struct RealProbe;

impl Probe for RealProbe {
    fn os(&self) -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Other
        }
    }

    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Option<String> {
        let mut cmd = Command::new(program);
        cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().ok()?;
        let mut out = child.stdout.take()?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            let _ = tx.send(s);
        });
        if let Ok(s) = rx.recv_timeout(timeout) {
            let ok = child.wait().is_ok_and(|st| st.success());
            ok.then_some(s)
        } else {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }

    fn drive_dirs(&self) -> Vec<PathBuf> {
        let Some(root) = dirs::home_dir().map(|h| h.join("Library/CloudStorage")) else {
            return Vec::new();
        };
        let mut out: Vec<PathBuf> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("GoogleDrive-")))
            .collect();
        out.sort();
        out
    }

    fn drive_app(&self) -> bool {
        match self.os() {
            Os::Mac => std::path::Path::new("/Applications/Google Drive.app").exists(),
            Os::Windows => std::path::Path::new(r"C:\Program Files\Google\Drive File Stream").exists(),
            Os::Other => false,
        }
    }
}

fn ps(p: &impl Probe, script: &str, timeout: Duration) -> Option<String> {
    p.run("powershell", &["-NoProfile", "-NonInteractive", "-Command", script], timeout)
}

// ---- parsers: macOS ------------------------------------------------------------------------------

/// `sysctl -n machdep.cpu.brand_string hw.ncpu hw.memsize` → (model, cores, bytes).
#[must_use]
pub fn parse_mac_sysctl(out: &str) -> (Option<String>, Option<u32>, Option<u64>) {
    let mut lines = out.lines().map(str::trim).filter(|l| !l.is_empty());
    let model = lines.next().map(str::to_owned);
    let cores = lines.next().and_then(|l| l.parse().ok());
    let mem = lines.next().and_then(|l| l.parse().ok());
    (model, cores, mem)
}

/// `vm_stat` → bytes a new job can use: free + inactive + speculative pages.
#[must_use]
pub fn parse_vm_stat(out: &str) -> Option<u64> {
    let first = out.lines().next()?;
    let page: u64 = first.split("page size of").nth(1)?.split_whitespace().next()?.parse().ok()?;
    let pages = |key: &str| -> u64 {
        out.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|r| r.trim().trim_end_matches('.').parse().ok())
            .unwrap_or(0)
    };
    let total = pages("Pages free:") + pages("Pages inactive:") + pages("Pages speculative:");
    (total > 0).then_some(total * page)
}

/// `system_profiler SPDisplaysDataType -json` → the GPUs (Apple Silicon reports `sppci_cores`).
#[must_use]
pub fn parse_mac_gpu(json: &str) -> Option<Vec<Gpu>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let list = v.get("SPDisplaysDataType")?.as_array()?;
    Some(
        list.iter()
            .filter_map(|d| {
                let name = ["sppci_model", "_name"].iter().find_map(|k| d.get(*k)?.as_str())?;
                let cores = d.get("sppci_cores").and_then(|c| match c {
                    Value::String(s) => s.trim().parse().ok(),
                    Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
                    _ => None,
                });
                Some(Gpu { name: name.chars().take(120).collect(), cores, vram_bytes: None })
            })
            .collect(),
    )
}

/// `pmset -g batt` → on mains or battery, and the charge when there is a battery.
#[must_use]
pub fn parse_pmset(out: &str) -> Option<Power> {
    let first = out.lines().next()?;
    let on_ac = if first.contains("AC Power") {
        true
    } else if first.contains("Battery Power") {
        false
    } else {
        return None;
    };
    let battery_percent = out.lines().skip(1).find_map(|l| {
        let end = l.find('%')?;
        let digits: String = l[..end].chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
        digits.parse().ok()
    });
    Some(Power { on_ac, battery_percent })
}

/// `ioreg -c IOHIDSystem` → seconds since the last key or mouse (the smallest `HIDIdleTime`, in ns).
#[must_use]
pub fn parse_hid_idle(out: &str) -> Option<f64> {
    out.lines()
        .filter_map(|l| {
            let (_, rest) = l.split_once("\"HIDIdleTime\" =")?;
            rest.trim().parse::<u64>().ok()
        })
        .min()
        .map(|ns| f(ns) / 1e9)
}

/// `du -sk <dir>` → bytes.
#[must_use]
pub fn parse_du_kb(out: &str) -> Option<u64> {
    out.split_whitespace().next()?.parse::<u64>().ok()?.checked_mul(1024)
}

/// `GoogleDrive-name@example.com` → `name@example.com`.
#[must_use]
pub fn drive_account(folder: &str) -> Option<String> {
    folder.strip_prefix("GoogleDrive-").filter(|s| !s.is_empty()).map(str::to_owned)
}

// ---- parsers: Windows ----------------------------------------------------------------------------

/// `PowerShell`'s `ConvertTo-Json` gives an object for one item and an array for several.
fn rows(json: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Array(a)) => a,
        Ok(v @ Value::Object(_)) => vec![v],
        _ => Vec::new(),
    }
}

fn num(v: &Value, key: &str) -> Option<f64> {
    match v.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// `Win32_Processor | Select Name,NumberOfLogicalProcessors` → the CPU.
#[must_use]
pub fn parse_win_cpu(json: &str) -> Option<Cpu> {
    let r = rows(json).into_iter().next()?;
    let model = r.get("Name")?.as_str()?.trim().to_owned();
    let cores = num(&r, "NumberOfLogicalProcessors")?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a core count
    Some(Cpu { model, cores: cores as u32 })
}

/// `Win32_OperatingSystem | Select TotalVisibleMemorySize,FreePhysicalMemory` (KB) → (total, free) bytes.
#[must_use]
pub fn parse_win_mem(json: &str) -> (Option<u64>, Option<u64>) {
    let Some(r) = rows(json).into_iter().next() else {
        return (None, None);
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // KB counts
    let kb = |k: &str| num(&r, k).map(|n| n as u64 * 1024);
    (kb("TotalVisibleMemorySize"), kb("FreePhysicalMemory"))
}

/// `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits` (MiB) → the GPUs.
#[must_use]
pub fn parse_nvidia_smi(out: &str) -> Vec<Gpu> {
    out.lines()
        .filter_map(|l| {
            let (name, mem) = l.rsplit_once(',')?;
            let mib: f64 = mem.trim().parse().ok()?;
            Some(Gpu { name: name.trim().chars().take(120).collect(), cores: None, vram_bytes: Some(mib * 1_048_576.0) })
        })
        .collect()
}

/// `Win32_VideoController | Select Name,AdapterRAM` → the GPUs (the fallback without nvidia-smi).
#[must_use]
pub fn parse_win_gpu(json: &str) -> Vec<Gpu> {
    rows(json)
        .iter()
        .filter_map(|r| {
            let name = r.get("Name")?.as_str()?.trim();
            (!name.is_empty()).then(|| Gpu { name: name.chars().take(120).collect(), cores: None, vram_bytes: num(r, "AdapterRAM").filter(|n| *n > 0.0) })
        })
        .collect()
}

/// `PowerStatus | Select PowerLineStatus,BatteryLifePercent` → power. A machine with no battery reads
/// `PowerLineStatus` 1 (Online) or 255 (Unknown, a desktop): both count as mains.
#[must_use]
pub fn parse_win_power(json: &str) -> Option<Power> {
    let r = rows(json).into_iter().next()?;
    let on_ac = match r.get("PowerLineStatus")? {
        Value::Number(n) => n.as_u64().is_some_and(|n| n != 0),
        Value::String(s) => !s.eq_ignore_ascii_case("offline"),
        _ => return None,
    };
    let battery_percent = num(&r, "BatteryLifePercent").filter(|p| (0.0..=1.0).contains(p)).map(|p| (p * 100.0).round());
    Some(Power { on_ac, battery_percent })
}

/// The one-line answer of the `GetLastInputInfo` script: milliseconds since the last input.
#[must_use]
pub fn parse_win_idle(out: &str) -> Option<f64> {
    out.trim().parse::<f64>().ok().filter(|ms| *ms >= 0.0).map(|ms| ms / 1000.0)
}

/// `Get-Volume … | Select Size,SizeRemaining` for the volume labelled "Google Drive" → used bytes.
#[must_use]
pub fn parse_win_drive(json: &str) -> Option<Drive> {
    let r = rows(json).into_iter().next()?;
    let used = match (num(&r, "Size"), num(&r, "SizeRemaining")) {
        (Some(size), Some(free)) if size >= free => Some(size - free),
        _ => None,
    };
    Some(Drive { installed: true, account: None, used_bytes: used })
}

const WIN_IDLE_SCRIPT: &str = "Add-Type 'using System;using System.Runtime.InteropServices;public class KunIdle{[StructLayout(LayoutKind.Sequential)]struct L{public uint s;public uint t;}[DllImport(\"user32.dll\")]static extern bool GetLastInputInfo(ref L p);public static uint Ms(){L l=new L();l.s=(uint)Marshal.SizeOf(l);GetLastInputInfo(ref l);return (uint)Environment.TickCount-l.t;}}'; [KunIdle]::Ms()";

// ---- readings ------------------------------------------------------------------------------------

/// Free memory a job could use right now.
pub fn ram_free(p: &impl Probe) -> Option<u64> {
    match p.os() {
        Os::Mac => parse_vm_stat(&p.run("vm_stat", &[], QUICK)?),
        Os::Windows => parse_win_mem(&ps(p, "Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize,FreePhysicalMemory | ConvertTo-Json -Compress", SLOW)?).1,
        Os::Other => None,
    }
}

/// On mains or battery, and the charge.
pub fn power(p: &impl Probe) -> Option<Power> {
    match p.os() {
        Os::Mac => parse_pmset(&p.run("pmset", &["-g", "batt"], QUICK)?),
        Os::Windows => parse_win_power(&ps(p, "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SystemInformation]::PowerStatus | Select-Object PowerLineStatus,BatteryLifePercent | ConvertTo-Json -Compress", SLOW)?),
        Os::Other => None,
    }
}

/// Seconds since the person last touched the keyboard or mouse.
pub fn idle_secs(p: &impl Probe) -> Option<f64> {
    match p.os() {
        Os::Mac => parse_hid_idle(&p.run("ioreg", &["-c", "IOHIDSystem"], QUICK)?),
        Os::Windows => parse_win_idle(&ps(p, WIN_IDLE_SCRIPT, SLOW)?),
        Os::Other => None,
    }
}

fn hardware(p: &impl Probe) -> (Option<Cpu>, Option<u64>, Option<Vec<Gpu>>) {
    match p.os() {
        Os::Mac => {
            let (model, cores, mem) = p
                .run("sysctl", &["-n", "machdep.cpu.brand_string", "hw.ncpu", "hw.memsize"], QUICK)
                .map(|o| parse_mac_sysctl(&o))
                .unwrap_or_default();
            let cpu = model.zip(cores).map(|(model, cores)| Cpu { model, cores });
            let gpu = p.run("system_profiler", &["SPDisplaysDataType", "-json"], SLOW).and_then(|o| parse_mac_gpu(&o));
            (cpu, mem, gpu)
        }
        Os::Windows => {
            let cpu = ps(p, "Get-CimInstance Win32_Processor | Select-Object -First 1 Name,NumberOfLogicalProcessors | ConvertTo-Json -Compress", SLOW).and_then(|o| parse_win_cpu(&o));
            let mem = ps(p, "Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize,FreePhysicalMemory | ConvertTo-Json -Compress", SLOW).and_then(|o| parse_win_mem(&o).0);
            let nvidia = p
                .run("nvidia-smi", &["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"], SLOW)
                .map(|o| parse_nvidia_smi(&o))
                .filter(|g| !g.is_empty());
            let gpu = nvidia.or_else(|| ps(p, "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json -Compress", SLOW).map(|o| parse_win_gpu(&o)));
            (cpu, mem, gpu)
        }
        Os::Other => (None, None, None),
    }
}

/// Google Drive: installed, the account (macOS: from the folder name) and the space it uses.
pub fn drive(p: &impl Probe) -> Option<Drive> {
    match p.os() {
        Os::Mac => {
            let dirs = p.drive_dirs();
            let Some(first) = dirs.first() else {
                return p.drive_app().then_some(Drive { installed: true, account: None, used_bytes: None });
            };
            let account = first.file_name().and_then(|n| drive_account(&n.to_string_lossy()));
            let used_bytes = p.run("du", &["-sk", &first.to_string_lossy()], DU_TIMEOUT).and_then(|o| parse_du_kb(&o)).map(f);
            Some(Drive { installed: true, account, used_bytes })
        }
        Os::Windows => {
            let script = "Get-Volume | Where-Object { $_.FileSystemLabel -eq 'Google Drive' } | Select-Object -First 1 Size,SizeRemaining | ConvertTo-Json -Compress";
            ps(p, script, SLOW)
                .and_then(|o| parse_win_drive(&o))
                .or_else(|| p.drive_app().then_some(Drive { installed: true, account: None, used_bytes: None }))
        }
        Os::Other => None,
    }
}

/// [`drive`], remembered for an hour in `cache` (`du` on a big Drive is slow).
pub fn drive_cached(p: &impl Probe, cache: &DriveCache, now: u64) -> Option<Drive> {
    {
        let held = cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, d)) = held.as_ref().filter(|(at, _)| now.saturating_sub(*at) < DRIVE_TTL_SECS) {
            return d.clone();
        }
    }
    let fresh = drive(p);
    *cache.lock().unwrap_or_else(PoisonError::into_inner) = Some((now, fresh.clone()));
    fresh
}

/// Everything the heartbeat reports. `disk_free` comes from the health facts; `internet` from the
/// heartbeat round trip and the hourly speed reading.
pub fn gather(p: &impl Probe, disk_free: Option<u64>, internet: Option<Internet>, drive: Option<Drive>) -> Resources {
    let (cpu, ram_total, gpu) = hardware(p);
    Resources {
        cpu,
        ram_total: ram_total.map(f),
        ram_free: ram_free(p).map(f),
        gpu,
        disk_free: disk_free.map(f),
        drive,
        internet,
        power: power(p),
        idle_secs: idle_secs(p),
    }
}

// ---- internet ------------------------------------------------------------------------------------

/// The Drive reading and when it was taken.
pub type DriveCache = Mutex<Option<(u64, Option<Drive>)>>;

/// What the last pass learned about the line.
#[derive(Debug, Clone, Default)]
pub struct NetState {
    pub latency_ms: Option<f64>,
    pub mbps_down: Option<f64>,
    pub measured_at: Option<String>,
    /// Unix second of the last speed reading.
    pub measured_unix: u64,
    /// Whether the last heartbeat reached kun.
    pub reached: Option<bool>,
}

pub static NET: LazyLock<Mutex<NetState>> = LazyLock::new(|| Mutex::new(NetState::default()));
static DRIVE: LazyLock<DriveCache> = LazyLock::new(|| Mutex::new(None));

#[must_use]
pub fn internet_now() -> Internet {
    let n = NET.lock().unwrap_or_else(PoisonError::into_inner).clone();
    Internet { online: n.reached.unwrap_or(true), latency_ms: n.latency_ms, mbps_down: n.mbps_down, measured_at: n.measured_at }
}

/// Megabits a second for `bytes` in `secs`.
#[must_use]
pub fn mbps(bytes: u64, secs: f64) -> Option<f64> {
    (bytes > 0 && secs > 0.0).then(|| (f(bytes) * 8.0 / secs / 1e6 * 10.0).round() / 10.0)
}

/// Times a 2 MB ranged download; `None` on any failure.
pub async fn measure_mbps() -> Option<f64> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).user_agent(concat!("kun-machine/", env!("CARGO_PKG_VERSION"))).build().ok()?;
    let start = std::time::Instant::now();
    let res = client.get(SPEED_URL).header(reqwest::header::RANGE, format!("bytes=0-{}", SPEED_BYTES - 1)).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let bytes = res.bytes().await.ok()?.len() as u64;
    mbps(bytes, start.elapsed().as_secs_f64())
}

/// Takes the hourly speed reading when it is due; `now_unix` and `now_iso` stamp it.
pub async fn refresh_speed(now_unix: u64, now_iso: &str) {
    let due = {
        let n = NET.lock().unwrap_or_else(PoisonError::into_inner);
        now_unix.saturating_sub(n.measured_unix) >= SPEED_EVERY_SECS
    };
    if !due {
        return;
    }
    let got = measure_mbps().await;
    let mut n = NET.lock().unwrap_or_else(PoisonError::into_inner);
    // A failed reading is retried next pass, but only after a short pause, not every 10 minutes forever hot.
    n.measured_unix = now_unix;
    if let Some(v) = got {
        n.mbps_down = Some(v);
        n.measured_at = Some(now_iso.to_owned());
    }
}

/// Records the heartbeat round trip (or its failure).
pub fn note_heartbeat(latency_ms: Option<f64>, reached: bool) {
    let mut n = NET.lock().unwrap_or_else(PoisonError::into_inner);
    n.reached = Some(reached);
    if latency_ms.is_some() {
        n.latency_ms = latency_ms;
    }
}

/// The real machine, now.
#[must_use]
pub fn real(disk_free: Option<u64>, now_unix: u64) -> Resources {
    gather(&RealProbe, disk_free, Some(internet_now()), drive_cached(&RealProbe, &DRIVE, now_unix))
}

#[cfg(test)]
#[path = "resources_test.rs"]
mod tests;
