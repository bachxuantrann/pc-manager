//! Đọc bảng SMBIOS (firmware) — BIOS, máy, mainboard, CPU, khe RAM.
//!
//! Phân tích thuần (không `unsafe`): mọi lần đọc dùng `get()`, dữ liệu firmware hỏng/thiếu chỉ làm
//! trường đó thành `None`, không bao giờ panic. Không đọc số serial (thông tin định danh máy).
//! Định dạng: DMTF SMBIOS Reference Specification (DSP0134).

use serde::Serialize;

/// Một cấu trúc SMBIOS: phần định dạng (`data`, gồm 4 byte header) + danh sách chuỗi.
struct Structure<'a> {
    kind: u8,
    data: &'a [u8],
    strings: Vec<&'a [u8]>,
}

impl Structure<'_> {
    fn byte(&self, off: usize) -> Option<u8> {
        self.data.get(off).copied()
    }

    fn word(&self, off: usize) -> Option<u16> {
        let b = self.data.get(off..off + 2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    fn dword(&self, off: usize) -> Option<u32> {
        let b = self.data.get(off..off + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Chuỗi theo chỉ số ở byte `off` (1-based; 0 = không có). Bỏ chuỗi rỗng / placeholder.
    fn string(&self, off: usize) -> Option<String> {
        let idx = usize::from(self.byte(off)?);
        let raw = self.strings.get(idx.checked_sub(1)?)?;
        let s = String::from_utf8_lossy(raw).trim().to_owned();
        let placeholder = ["", "To Be Filled By O.E.M.", "Default string", "Not Specified", "System Product Name", "Unknown", "x.x", "None"];
        (!placeholder.iter().any(|p| s.eq_ignore_ascii_case(p))).then_some(s)
    }
}

/// Tách bảng thành các cấu trúc. Dừng ở cấu trúc type 127 (End) hoặc khi dữ liệu hỏng.
fn structures(table: &[u8]) -> Vec<Structure<'_>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(&[kind, len]) = table.get(pos..pos + 2).and_then(|h| <&[u8; 2]>::try_from(h).ok()) {
        let len = usize::from(len);
        if len < 4 {
            break;
        }
        let Some(data) = table.get(pos..pos + len) else { break };
        // Vùng chuỗi: các chuỗi kết thúc NUL, cả vùng kết thúc bằng hai NUL liên tiếp.
        let mut end = pos + len;
        let mut strings = Vec::new();
        loop {
            let rest = table.get(end..).unwrap_or_default();
            let Some(n) = rest.iter().position(|&b| b == 0) else { return out };
            if n == 0 {
                // Chuỗi rỗng: hết vùng chuỗi (nếu chưa có chuỗi nào thì có thêm 1 NUL nữa).
                end += if strings.is_empty() { 2 } else { 1 };
                break;
            }
            strings.push(&rest[..n]);
            end += n + 1;
        }
        out.push(Structure { kind, data, strings });
        if kind == 127 {
            break;
        }
        pos = end;
    }
    out
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Bios {
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    /// Hãng máy (type 1) — vd Dell, Lenovo; máy tự ráp thường trống.
    pub system_manufacturer: Option<String>,
    pub system_product: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Cpu {
    pub socket: Option<String>,
    pub name: Option<String>,
    pub cores: Option<u16>,
    pub threads: Option<u16>,
    pub max_mhz: Option<u16>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemorySlot {
    /// Tên khe, vd "DIMM A1" / "ChannelA-DIMM0".
    pub locator: Option<String>,
    pub bank: Option<String>,
    /// None = khe trống.
    pub size_mb: Option<u64>,
    pub kind: Option<String>,
    /// MT/s tối đa của thanh RAM.
    pub speed_mts: Option<u32>,
    /// MT/s đang chạy (có thể thấp hơn nếu chưa bật XMP/EXPO).
    pub configured_mts: Option<u32>,
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub form_factor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Smbios {
    pub bios: Bios,
    pub board: Board,
    pub cpus: Vec<Cpu>,
    /// Tổng số khe RAM mainboard có (type 16), có thể lớn hơn số mục `slots` firmware liệt kê.
    pub ram_slots_total: Option<u32>,
    pub slots: Vec<MemorySlot>,
}

fn memory_kind(code: u8) -> Option<&'static str> {
    Some(match code {
        0x12 => "DDR",
        0x13 => "DDR2",
        0x18 => "DDR3",
        0x1A => "DDR4",
        0x1B => "LPDDR",
        0x1C => "LPDDR2",
        0x1D => "LPDDR3",
        0x1E => "LPDDR4",
        0x20 => "HBM",
        0x21 => "HBM2",
        0x22 => "DDR5",
        0x23 => "LPDDR5",
        _ => return None,
    })
}

fn form_factor(code: u8) -> Option<&'static str> {
    Some(match code {
        0x09 => "DIMM",
        0x0D => "SODIMM",
        0x0F => "FB-DIMM",
        0x10 => "Die",
        _ => return None,
    })
}

/// Dung lượng một thanh RAM (type 17): `None` = khe trống / không rõ.
fn module_size_mb(s: &Structure<'_>) -> Option<u64> {
    match s.word(0x0C)? {
        0 | 0xFFFF => None,
        0x7FFF => s.dword(0x1C).map(|mb| u64::from(mb & 0x7FFF_FFFF)).filter(|&mb| mb > 0),
        // Bit 15 = đơn vị KB thay vì MB.
        v if v & 0x8000 != 0 => Some(u64::from(v & 0x7FFF) / 1024).filter(|&mb| mb > 0),
        v => Some(u64::from(v)),
    }
}

/// Tốc độ: giá trị 16-bit; 0xFFFF => xem trường 32-bit mở rộng.
fn speed(s: &Structure<'_>, off16: usize, off32: usize) -> Option<u32> {
    match s.word(off16)? {
        0 => None,
        0xFFFF => s.dword(off32).filter(|&v| v > 0),
        v => Some(u32::from(v)),
    }
}

/// Phân tích bảng SMBIOS (phần dữ liệu, không gồm header 8 byte của `GetSystemFirmwareTable`).
pub fn parse(table: &[u8]) -> Smbios {
    let mut out = Smbios::default();
    for s in structures(table) {
        match s.kind {
            0 => {
                out.bios = Bios { vendor: s.string(0x04), version: s.string(0x05), release_date: s.string(0x08) };
            }
            1 => {
                out.board.system_manufacturer = s.string(0x04);
                out.board.system_product = s.string(0x05);
            }
            2 => {
                // Chỉ lấy mainboard đầu tiên.
                if out.board.manufacturer.is_none() && out.board.product.is_none() {
                    out.board.manufacturer = s.string(0x04);
                    out.board.product = s.string(0x05);
                    out.board.version = s.string(0x06);
                }
            }
            4 => {
                let cores = match s.byte(0x23) {
                    Some(0xFF) => s.word(0x2A),
                    Some(0) | None => None,
                    Some(v) => Some(u16::from(v)),
                };
                let threads = match s.byte(0x25) {
                    Some(0xFF) => s.word(0x2E),
                    Some(0) | None => None,
                    Some(v) => Some(u16::from(v)),
                };
                // Socket trống (status bit 6 = 0) thì bỏ.
                let populated = s.byte(0x18).is_none_or(|st| st & 0x40 != 0);
                if populated {
                    out.cpus.push(Cpu {
                        socket: s.string(0x04),
                        name: s.string(0x10),
                        cores,
                        threads,
                        max_mhz: s.word(0x14).filter(|&v| v > 0),
                    });
                }
            }
            16 => {
                // Use = 3 (system memory); bỏ mảng nhớ cache/video.
                if s.byte(0x05) == Some(3) {
                    let n = s.word(0x0D).map(u32::from).unwrap_or(0);
                    out.ram_slots_total = Some(out.ram_slots_total.unwrap_or(0) + n);
                }
            }
            17 => {
                let size_mb = module_size_mb(&s);
                out.slots.push(MemorySlot {
                    locator: s.string(0x10),
                    bank: s.string(0x11),
                    kind: size_mb.and(s.byte(0x12)).and_then(memory_kind).map(str::to_owned),
                    speed_mts: size_mb.and(speed(&s, 0x15, 0x54)),
                    configured_mts: size_mb.and(speed(&s, 0x20, 0x58)),
                    manufacturer: size_mb.and(s.string(0x17)),
                    part_number: size_mb.and(s.string(0x1A)),
                    form_factor: size_mb.and(s.byte(0x0E)).and_then(form_factor).map(str::to_owned),
                    size_mb,
                });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ghép một cấu trúc: header (type, len, handle) + phần định dạng + chuỗi.
    fn structure(kind: u8, body: &[u8], strings: &[&str]) -> Vec<u8> {
        let len = u8::try_from(4 + body.len()).unwrap();
        let mut v = vec![kind, len, 0, 0];
        v.extend_from_slice(body);
        if strings.is_empty() {
            v.extend_from_slice(&[0, 0]);
        } else {
            for s in strings {
                v.extend_from_slice(s.as_bytes());
                v.push(0);
            }
            v.push(0);
        }
        v
    }

    fn put(body: &mut Vec<u8>, off: usize, bytes: &[u8]) {
        let i = off - 4;
        if body.len() < i + bytes.len() {
            body.resize(i + bytes.len(), 0);
        }
        body[i..i + bytes.len()].copy_from_slice(bytes);
    }

    fn sample() -> Vec<u8> {
        let mut t = Vec::new();
        // BIOS: vendor=1, version=2, date=3
        let mut b0 = Vec::new();
        put(&mut b0, 0x04, &[1, 2]);
        put(&mut b0, 0x08, &[3]);
        t.extend(structure(0, &b0, &["American Megatrends", "1.20", "03/15/2025"]));
        // System: placeholder sản phẩm => bị bỏ.
        let mut b1 = Vec::new();
        put(&mut b1, 0x04, &[1, 2]);
        t.extend(structure(1, &b1, &["ASUS", "System Product Name"]));
        // Baseboard
        let mut b2 = Vec::new();
        put(&mut b2, 0x04, &[1, 2, 3]);
        t.extend(structure(2, &b2, &["ASUSTeK COMPUTER INC.", "ROG STRIX B650-A", "Rev 1.xx"]));
        // CPU: socket=1, name=2, 16 nhân, 32 luồng, 5700 MHz, socket có CPU.
        let mut b4 = Vec::new();
        put(&mut b4, 0x04, &[1]);
        put(&mut b4, 0x10, &[2]);
        put(&mut b4, 0x14, &5700u16.to_le_bytes());
        put(&mut b4, 0x18, &[0x41]);
        put(&mut b4, 0x23, &[16]);
        put(&mut b4, 0x25, &[32]);
        t.extend(structure(4, &b4, &["AM5", "AMD Ryzen 9 9950X 16-Core Processor"]));
        // Mảng nhớ hệ thống: 4 khe.
        let mut b16 = Vec::new();
        put(&mut b16, 0x05, &[3]);
        put(&mut b16, 0x0D, &4u16.to_le_bytes());
        t.extend(structure(16, &b16, &[]));
        // Thanh RAM 16 GB DDR5 6000/4800.
        let mut b17 = Vec::new();
        put(&mut b17, 0x0C, &16384u16.to_le_bytes());
        put(&mut b17, 0x0E, &[0x09]);
        put(&mut b17, 0x10, &[1, 2, 0x22]);
        put(&mut b17, 0x15, &6000u16.to_le_bytes());
        put(&mut b17, 0x17, &[3]);
        put(&mut b17, 0x1A, &[4]);
        put(&mut b17, 0x20, &4800u16.to_le_bytes());
        t.extend(structure(17, &b17, &["DIMM_A2", "BANK 0", "Kingston", "KF560C36-16"]));
        // Khe trống.
        let mut e17 = Vec::new();
        put(&mut e17, 0x0C, &0u16.to_le_bytes());
        put(&mut e17, 0x10, &[1]);
        t.extend(structure(17, &e17, &["DIMM_A1"]));
        t.extend(structure(127, &[], &[]));
        t
    }

    #[test]
    fn parses_bios_board_cpu_and_ram() {
        let s = parse(&sample());
        assert_eq!(s.bios.vendor.as_deref(), Some("American Megatrends"));
        assert_eq!(s.bios.release_date.as_deref(), Some("03/15/2025"));
        assert_eq!(s.board.system_manufacturer.as_deref(), Some("ASUS"));
        assert_eq!(s.board.system_product, None, "placeholder bị bỏ");
        assert_eq!(s.board.product.as_deref(), Some("ROG STRIX B650-A"));
        let cpu = &s.cpus[0];
        assert_eq!((cpu.cores, cpu.threads, cpu.max_mhz), (Some(16), Some(32), Some(5700)));
        assert_eq!(s.ram_slots_total, Some(4));
        assert_eq!(s.slots.len(), 2);
        let m = &s.slots[0];
        assert_eq!(m.size_mb, Some(16384));
        assert_eq!(m.kind.as_deref(), Some("DDR5"));
        assert_eq!((m.speed_mts, m.configured_mts), (Some(6000), Some(4800)));
        assert_eq!(m.part_number.as_deref(), Some("KF560C36-16"));
        assert_eq!(s.slots[1].size_mb, None, "khe trống");
        assert_eq!(s.slots[1].locator.as_deref(), Some("DIMM_A1"));
    }

    #[test]
    fn corrupt_tables_never_panic() {
        let full = sample();
        // Cắt ở mọi vị trí và làm hỏng từng byte: không panic, chỉ thiếu dữ liệu.
        for n in 0..full.len() {
            let _ = parse(&full[..n]);
            let mut bad = full.clone();
            bad[n] = 0xFF;
            let _ = parse(&bad);
        }
        assert_eq!(parse(&[]), Smbios::default());
    }

    #[test]
    fn extended_and_kb_sizes() {
        let mut ext = Vec::new();
        put(&mut ext, 0x0C, &0x7FFFu16.to_le_bytes());
        put(&mut ext, 0x1C, &65536u32.to_le_bytes());
        let mut kb = Vec::new();
        put(&mut kb, 0x0C, &(0x8000u16 | 2048).to_le_bytes());
        let mut t = structure(17, &ext, &[]);
        t.extend(structure(17, &kb, &[]));
        let s = parse(&t);
        assert_eq!(s.slots[0].size_mb, Some(65536));
        assert_eq!(s.slots[1].size_mb, Some(2));
    }
}
