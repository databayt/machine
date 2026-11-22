use std::{collections::HashMap, sync::Mutex};

use super::*;

const SYSCTL: &str = "Apple M4 Pro\n14\n51539607552\n";

const VM_STAT: &str = "Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                               12000.
Pages active:                            900000.
Pages inactive:                          600000.
Pages speculative:                         4000.
Pages throttled:                              0.
Pages wired down:                        200000.
";

const SPDISPLAYS: &str = r#"{
  "SPDisplaysDataType" : [
    {
      "_name" : "Apple M4 Pro",
      "spdisplays_mtlgpufamilysupport" : "spdisplays_metal4",
      "sppci_bus" : "spdisplays_builtin",
      "sppci_cores" : "20",
      "sppci_device_type" : "spdisplays_gpu",
      "sppci_model" : "Apple M4 Pro",
      "spdisplays_vendor" : "sppci_vendor_Apple"
    }
  ]
}"#;

const PMSET_AC: &str = "Now drawing from 'AC Power'
 -InternalBattery-0 (id=21954659)\t87%; charging; 0:42 remaining present: true
";
const PMSET_BATT: &str = "Now drawing from 'Battery Power'
 -InternalBattery-0 (id=21954659)\t54%; discharging; 3:10 remaining present: true
";
const PMSET_DESKTOP: &str = "Now drawing from 'AC Power'\n";

const IOREG: &str = r#"+-o IOHIDSystem  <class IOHIDSystem, id 0x100000321, registered, matched, active, busy 0 (0 ms), retain 8>
    {
      "HIDIdleTime" = 4200000000
      "IOClass" = "IOHIDSystem"
    }
+-o IOHIDSystem  <class IOHIDSystem, id 0x100000999>
    {
      "HIDIdleTime" = 1500000000
    }
"#;

const WIN_CPU: &str = r#"{"Name":"AMD Ryzen 9 7950X 16-Core Processor        ","NumberOfLogicalProcessors":32}"#;
const WIN_MEM: &str = r#"{"TotalVisibleMemorySize":33447132,"FreePhysicalMemory":20123456}"#;
const NVIDIA: &str = "NVIDIA GeForce RTX 4090, 24564\n";
const WIN_GPU_TWO: &str = r#"[{"Name":"Intel(R) UHD Graphics","AdapterRAM":1073741824},{"Name":"Microsoft Basic Display Adapter","AdapterRAM":null}]"#;
const WIN_POWER_BATT: &str = r#"{"PowerLineStatus":0,"BatteryLifePercent":0.42}"#;
const WIN_POWER_AC: &str = r#"{"PowerLineStatus":1,"BatteryLifePercent":1.0}"#;
const WIN_POWER_DESKTOP: &str = r#"{"PowerLineStatus":255,"BatteryLifePercent":255.0}"#;
const WIN_DRIVE: &str = r#"{"Size":107374182400,"SizeRemaining":32212254720}"#;

#[test]
fn macos_hardware_parses() {
    assert_eq!(parse_mac_sysctl(SYSCTL), (Some("Apple M4 Pro".into()), Some(14), Some(51_539_607_552)));
    assert_eq!(parse_mac_sysctl(""), (None, None, None));
    // free + inactive + speculative pages x 16 KB
    assert_eq!(parse_vm_stat(VM_STAT), Some((12_000 + 600_000 + 4_000) * 16_384));
    assert_eq!(parse_vm_stat("garbage"), None);
    let gpu = parse_mac_gpu(SPDISPLAYS).unwrap();
    assert_eq!(gpu, vec![Gpu { name: "Apple M4 Pro".into(), cores: Some(20), vram_bytes: None }]);
    assert_eq!(parse_mac_gpu("not json"), None);
}

#[test]
fn macos_power_idle_and_du_parse() {
    assert_eq!(parse_pmset(PMSET_AC), Some(Power { on_ac: true, battery_percent: Some(87.0) }));
    assert_eq!(parse_pmset(PMSET_BATT), Some(Power { on_ac: false, battery_percent: Some(54.0) }));
    assert_eq!(parse_pmset(PMSET_DESKTOP), Some(Power { on_ac: true, battery_percent: None }));
    assert_eq!(parse_pmset("pmset: nothing"), None);
    // the smaller of the two idle readings, ns to s
    assert!((parse_hid_idle(IOREG).unwrap() - 1.5).abs() < 1e-9);
    assert_eq!(parse_hid_idle("nothing"), None);
    assert_eq!(parse_du_kb("2048\t/Users/x/Library/CloudStorage/GoogleDrive-a@b.org\n"), Some(2_097_152));
    assert_eq!(parse_du_kb(""), None);
    assert_eq!(drive_account("GoogleDrive-osman@databayt.org"), Some("osman@databayt.org".into()));
    assert_eq!(drive_account("Dropbox"), None);
}

#[test]
fn windows_readings_parse() {
    assert_eq!(parse_win_cpu(WIN_CPU), Some(Cpu { model: "AMD Ryzen 9 7950X 16-Core Processor".into(), cores: 32 }));
    assert_eq!(parse_win_mem(WIN_MEM), (Some(33_447_132 * 1024), Some(20_123_456 * 1024)));
    let g = parse_nvidia_smi(NVIDIA);
    assert_eq!(g[0].name, "NVIDIA GeForce RTX 4090");
    assert_eq!(g[0].vram_bytes, Some(24_564.0 * 1_048_576.0));
    let g = parse_win_gpu(WIN_GPU_TWO);
    assert_eq!(g.len(), 2);
    assert_eq!(g[0].vram_bytes, Some(1_073_741_824.0));
    assert_eq!(g[1].vram_bytes, None);
    // one item is an object, several an array: both read
    assert_eq!(parse_win_gpu(r#"{"Name":"X","AdapterRAM":5}"#).len(), 1);
    assert_eq!(parse_win_power(WIN_POWER_BATT), Some(Power { on_ac: false, battery_percent: Some(42.0) }));
    assert_eq!(parse_win_power(WIN_POWER_AC), Some(Power { on_ac: true, battery_percent: Some(100.0) }));
    assert_eq!(parse_win_power(WIN_POWER_DESKTOP), Some(Power { on_ac: true, battery_percent: None }));
    assert_eq!(parse_win_idle("90000\r\n"), Some(90.0));
    assert_eq!(parse_win_idle("x"), None);
    let d = parse_win_drive(WIN_DRIVE).unwrap();
    assert_eq!(d.used_bytes, Some(107_374_182_400.0 - 32_212_254_720.0));
}

/// A scripted machine: commands answer from a map keyed by `program args`.
struct Fake {
    os: Os,
    out: HashMap<String, String>,
    dirs: Vec<PathBuf>,
    app: bool,
    calls: Mutex<u32>,
}

impl Fake {
    fn new(os: Os) -> Self {
        Self { os, out: HashMap::new(), dirs: Vec::new(), app: false, calls: Mutex::new(0) }
    }
    fn with(mut self, cmd: &str, out: &str) -> Self {
        self.out.insert(cmd.to_owned(), out.to_owned());
        self
    }
}

impl Probe for Fake {
    fn os(&self) -> Os {
        self.os
    }
    fn run(&self, program: &str, args: &[&str], _t: Duration) -> Option<String> {
        *self.calls.lock().unwrap() += 1;
        let key = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        // PowerShell scripts are matched by their first words.
        self.out.iter().find(|(k, _)| key.starts_with(k.as_str())).map(|(_, v)| v.clone())
    }
    fn drive_dirs(&self) -> Vec<PathBuf> {
        self.dirs.clone()
    }
    fn drive_app(&self) -> bool {
        self.app
    }
}

fn mac() -> Fake {
    Fake::new(Os::Mac)
        .with("sysctl -n machdep.cpu.brand_string hw.ncpu hw.memsize", SYSCTL)
        .with("vm_stat", VM_STAT)
        .with("system_profiler SPDisplaysDataType -json", SPDISPLAYS)
        .with("pmset -g batt", PMSET_AC)
        .with("ioreg -c IOHIDSystem", IOREG)
}

#[test]
fn a_mac_gathers_everything_and_the_one_line_summary_fields_are_there() {
    let r = gather(&mac(), Some(120 << 30), Some(Internet { online: true, latency_ms: Some(40.0), mbps_down: Some(88.4), measured_at: None }), None);
    assert_eq!(r.cpu, Some(Cpu { model: "Apple M4 Pro".into(), cores: 14 }));
    assert_eq!(r.ram_total, Some(51_539_607_552.0));
    assert!(r.ram_free.unwrap() > 9e9);
    assert_eq!(r.gpu.as_ref().unwrap()[0].cores, Some(20));
    assert_eq!(r.disk_free, Some(f(120 << 30)));
    assert!(r.power.as_ref().unwrap().on_ac);
    assert!((r.idle_secs.unwrap() - 1.5).abs() < 1e-9);
    // The wire is camelCase and drops what is unknown.
    let json = serde_json::to_value(&r).unwrap();
    assert!(json.get("ramTotal").is_some() && json.get("idleSecs").is_some());
    assert!(json.get("drive").is_none());
}

#[test]
fn a_failed_read_leaves_the_field_out() {
    let r = gather(&Fake::new(Os::Mac), None, None, None);
    assert_eq!(r, Resources::default());
    assert_eq!(serde_json::to_string(&r).unwrap(), "{}");
}

#[test]
fn a_windows_pc_gathers_through_powershell() {
    let p = Fake::new(Os::Windows)
        .with("powershell -NoProfile -NonInteractive -Command Get-CimInstance Win32_Processor", WIN_CPU)
        .with("powershell -NoProfile -NonInteractive -Command Get-CimInstance Win32_OperatingSystem", WIN_MEM)
        .with("nvidia-smi", NVIDIA)
        .with("powershell -NoProfile -NonInteractive -Command Add-Type -AssemblyName", WIN_POWER_BATT)
        .with("powershell -NoProfile -NonInteractive -Command Add-Type 'using", "61000");
    let r = gather(&p, None, None, None);
    assert_eq!(r.cpu.as_ref().unwrap().cores, 32);
    assert_eq!(r.ram_free, Some(f(20_123_456 * 1024)));
    assert_eq!(r.gpu.as_ref().unwrap()[0].name, "NVIDIA GeForce RTX 4090");
    assert!(!r.power.as_ref().unwrap().on_ac);
    assert_eq!(r.idle_secs, Some(61.0));
}

#[test]
fn the_drive_reads_the_account_from_the_folder_and_is_cached_for_an_hour() {
    let mut p = mac().with("du -sk", "1024\t/x\n");
    p.dirs = vec![PathBuf::from("/Users/x/Library/CloudStorage/GoogleDrive-osman@databayt.org")];
    let d = drive(&p).unwrap();
    assert_eq!(d.account.as_deref(), Some("osman@databayt.org"));
    assert_eq!(d.used_bytes, Some(1_048_576.0));
    let cache = Mutex::new(None);
    let before = *p.calls.lock().unwrap();
    let a = drive_cached(&p, &cache, 1000);
    let b = drive_cached(&p, &cache, 1000 + 3599);
    assert_eq!(a, b);
    assert_eq!(*p.calls.lock().unwrap(), before + 1, "du ran once inside the hour");
    let _ = drive_cached(&p, &cache, 1000 + 3600);
    assert_eq!(*p.calls.lock().unwrap(), before + 2, "and again after it");
}

#[test]
fn no_drive_folder_means_installed_only_when_the_app_is_there() {
    let mut p = mac();
    assert_eq!(drive(&p), None);
    p.app = true;
    assert_eq!(drive(&p), Some(Drive { installed: true, account: None, used_bytes: None }));
    let d = parse_win_drive("{}").unwrap();
    assert_eq!(d.used_bytes, None);
}

#[test]
fn speed_is_megabits_a_second() {
    // 2 MiB in 1 s is 16.8 Mbit/s
    assert_eq!(mbps(2 * 1024 * 1024, 1.0), Some(16.8));
    assert_eq!(mbps(0, 1.0), None);
    assert_eq!(mbps(10, 0.0), None);
}
