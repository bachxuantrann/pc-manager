//! Thông tin phần cứng & Windows (trang "Cấu hình máy"). Đọc một lần khi mở trang: SMBIOS,
//! IOCTL ổ đĩa, registry — không WMI, không DXGI (không nạp DLL đồ hoạ vào tiến trình), không chạy nền.
//! Không thu thập số serial / tên máy / tên chủ sở hữu (thông tin định danh).

pub mod smbios;

use serde::Serialize;
use windows_registry::{Key, LOCAL_MACHINE};

use crate::sys;
use smbios::{Bios, Board, MemorySlot};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsInfo {
    /// vd "Windows 11 Pro".
    pub product: String,
    /// vd "24H2".
    pub display_version: Option<String>,
    /// vd "26100.4061".
    pub build: Option<String>,
    pub install_date_ms: Option<i64>,
    pub uptime_secs: u64,
    pub architecture: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: Option<String>,
    pub socket: Option<String>,
    pub cores: Option<u16>,
    pub threads: Option<u16>,
    /// Xung cơ bản (registry `~MHz`).
    pub base_mhz: Option<u32>,
    /// Xung tối đa firmware khai báo (SMBIOS) — không phải xung boost thực tế.
    pub max_mhz: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: Option<String>,
    pub vram_bytes: Option<u64>,
    pub driver_version: Option<String>,
    pub driver_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    /// Tổng số khe trên mainboard (nếu firmware khai báo).
    pub slots_total: Option<u32>,
    pub slots: Vec<MemorySlot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub index: u32,
    pub model: String,
    pub size_bytes: Option<u64>,
    /// "NVMe", "SATA", "USB"…
    pub bus: Option<&'static str>,
    /// "SSD" / "HDD" (theo thời gian tìm vết; ổ USB/ảo có thể không rõ).
    pub media: Option<&'static str>,
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub windows: WindowsInfo,
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub board: Board,
    pub bios: Bios,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
}

fn reg_string(k: &Key, name: &str) -> Option<String> {
    k.get_string(name).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Windows 11 vẫn ghi "Windows 10" trong `ProductName`: sửa theo số build (từ 22000 là Windows 11).
fn product_name(raw: &str, build: Option<u32>) -> String {
    match (raw.strip_prefix("Windows 10"), build) {
        (Some(rest), Some(b)) if b >= 22_000 => format!("Windows 11{rest}"),
        _ => raw.to_owned(),
    }
}

fn windows_info() -> WindowsInfo {
    let key = LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").ok();
    let s = |n: &str| key.as_ref().and_then(|k| reg_string(k, n));
    let build_no = s("CurrentBuildNumber").and_then(|b| b.parse::<u32>().ok());
    let ubr = key.as_ref().and_then(|k| k.get_u32("UBR").ok());
    WindowsInfo {
        product: product_name(&s("ProductName").unwrap_or_else(|| "Windows".into()), build_no),
        display_version: s("DisplayVersion").or_else(|| s("ReleaseId")),
        build: build_no.map(|b| ubr.map_or_else(|| b.to_string(), |u| format!("{b}.{u}"))),
        install_date_ms: key.as_ref().and_then(|k| k.get_u32("InstallDate").ok()).filter(|&t| t > 0).map(|t| i64::from(t) * 1000),
        uptime_secs: sys::firmware::uptime_secs(),
        architecture: if cfg!(target_arch = "x86_64") { "x64" } else { "khác" },
    }
}

fn cpu_info(smb: Option<&smbios::Cpu>) -> CpuInfo {
    let key = LOCAL_MACHINE.open(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0").ok();
    let threads_os = std::thread::available_parallelism().ok().and_then(|n| u16::try_from(n.get()).ok());
    CpuInfo {
        // Tên trong registry chuẩn hơn chuỗi firmware (có khi là "Intel(R) Core(TM)..." đầy đủ).
        name: key.as_ref().and_then(|k| reg_string(k, "ProcessorNameString")).or_else(|| smb.and_then(|c| c.name.clone())),
        socket: smb.and_then(|c| c.socket.clone()),
        cores: smb.and_then(|c| c.cores),
        threads: smb.and_then(|c| c.threads).or(threads_os),
        base_mhz: key.as_ref().and_then(|k| k.get_u32("~MHz").ok()).filter(|&m| m > 0),
        max_mhz: smb.and_then(|c| c.max_mhz),
    }
}

/// Lớp thiết bị Display Adapters.
const DISPLAY_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Card màn hình ảo / từ xa — không phải GPU thật.
fn is_virtual_adapter(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["remote display", "basic render", "indirect display", "virtual display", "parsec", "meta virtual"].iter().any(|p| n.contains(p))
}

fn vram(k: &Key) -> Option<u64> {
    if let Ok(v) = k.get_u64("HardwareInformation.qwMemorySize") {
        return Some(v).filter(|&v| v > 0);
    }
    // Driver cũ: DWORD hoặc 4 byte nhị phân.
    if let Ok(v) = k.get_u32("HardwareInformation.MemorySize") {
        return Some(u64::from(v)).filter(|&v| v > 0);
    }
    let raw = k.get_value("HardwareInformation.MemorySize").ok()?;
    let b: [u8; 4] = raw.get(..4)?.try_into().ok()?;
    Some(u64::from(u32::from_le_bytes(b))).filter(|&v| v > 0)
}

fn gpus() -> Vec<GpuInfo> {
    let Ok(class) = LOCAL_MACHINE.open(DISPLAY_CLASS) else { return Vec::new() };
    let Ok(subs) = class.keys() else { return Vec::new() };
    let mut out: Vec<GpuInfo> = Vec::new();
    for sub in subs.filter(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit())) {
        let Ok(k) = class.open(&sub) else { continue };
        let Some(name) = reg_string(&k, "DriverDesc") else { continue };
        if is_virtual_adapter(&name) || out.iter().any(|g| g.name == name) {
            continue;
        }
        out.push(GpuInfo {
            vendor: reg_string(&k, "ProviderName"),
            vram_bytes: vram(&k),
            driver_version: reg_string(&k, "DriverVersion"),
            driver_date: reg_string(&k, "DriverDate"),
            name,
        });
    }
    out
}

fn bus_name(code: i32) -> Option<&'static str> {
    Some(match code {
        1 => "SCSI",
        3 => "ATA",
        7 => "USB",
        8 => "RAID",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        13 => "MMC",
        14 | 15 => "Ổ ảo",
        16 => "Storage Spaces",
        17 => "NVMe",
        18 => "SCM",
        19 => "UFS",
        _ => return None,
    })
}

fn disks() -> Vec<DiskInfo> {
    sys::storage::physical_disks()
        .into_iter()
        .map(|d| {
            // Nhiều ổ NVMe để vendor trống và tên đầy đủ ở product; ghép nếu vendor chưa có trong product.
            let model = match (d.vendor.as_deref(), d.product.as_deref()) {
                (Some(v), Some(p)) if !p.to_ascii_lowercase().contains(&v.to_ascii_lowercase()) => format!("{v} {p}"),
                (_, Some(p)) => p.to_owned(),
                (Some(v), None) => v.to_owned(),
                (None, None) => format!("Ổ đĩa {}", d.index),
            };
            let bus = d.bus_type.and_then(bus_name);
            DiskInfo {
                index: d.index,
                model,
                size_bytes: d.size_bytes,
                // NVMe luôn là SSD; ổ USB báo tìm vết không đáng tin.
                media: match (bus, d.seek_penalty) {
                    (Some("NVMe"), _) => Some("SSD"),
                    (Some("USB"), _) | (_, None) => None,
                    (_, Some(true)) => Some("HDD"),
                    (_, Some(false)) => Some("SSD"),
                },
                bus,
                firmware: d.revision,
            }
        })
        .collect()
}

/// Đọc toàn bộ thông tin (vài chục ms). Phần nào lỗi thì để trống, không làm hỏng phần khác.
pub fn collect() -> SystemInfo {
    let smb = sys::firmware::smbios_table().map(|t| smbios::parse(&t)).unwrap_or_default();
    SystemInfo {
        windows: windows_info(),
        cpu: cpu_info(smb.cpus.first()),
        gpus: gpus(),
        memory: MemoryInfo {
            total_bytes: sys::memory::basic_memory().map(|m| m.total_phys).unwrap_or(0),
            slots_total: smb.ram_slots_total,
            slots: smb.slots,
        },
        board: smb.board,
        bios: smb.bios,
        disks: disks(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_11_product_name_is_fixed_by_build() {
        assert_eq!(product_name("Windows 10 Pro", Some(26100)), "Windows 11 Pro");
        assert_eq!(product_name("Windows 10 Pro", Some(19045)), "Windows 10 Pro");
        assert_eq!(product_name("Windows 10 Enterprise", None), "Windows 10 Enterprise");
        assert_eq!(product_name("Windows Server 2022", Some(20348)), "Windows Server 2022");
    }

    #[test]
    fn virtual_adapters_are_skipped() {
        assert!(is_virtual_adapter("Microsoft Remote Display Adapter"));
        assert!(is_virtual_adapter("Microsoft Basic Render Driver"));
        assert!(!is_virtual_adapter("NVIDIA GeForce RTX 4090"));
    }

    #[test]
    fn collects_this_machine_without_admin() {
        let i = collect();
        assert!(i.windows.product.starts_with("Windows"));
        assert!(i.windows.build.is_some());
        assert!(i.cpu.name.is_some());
        assert!(i.memory.total_bytes > 0);
        assert!(!i.disks.is_empty());
        // In ra để đối chiếu bằng mắt khi chạy `cargo test collects -- --nocapture`.
        println!("{}", serde_json::to_string_pretty(&i).unwrap_or_default());
    }
}
