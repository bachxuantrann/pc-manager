import { IconCheck, IconRefresh } from "../components/Icons";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import Button from "../components/Button";
import { Card, Notice } from "../components/Card";
import PageShell from "../components/PageShell";
import ProgressBar from "../components/ProgressBar";
import { api, errorMessage, type MemorySlot, type SystemInfo } from "../lib/api";
import { formatBytes, formatDate } from "../lib/format";

/** Dung lượng ổ theo cách nhà sản xuất ghi (1 GB = 1000³ byte): ổ "256GB" hiện 256 GB, không phải 238. */
function diskSize(n: number | null): string {
  if (n == null) return "—";
  const tb = n / 1e12;
  return tb >= 1 ? `${tb.toFixed(tb >= 10 ? 0 : 1)} TB` : `${Math.round(n / 1e9)} GB`;
}

const ghz = (mhz: number | null) => (mhz ? `${(mhz / 1000).toFixed(1)} GHz` : null);

function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return d > 0 ? `${d} ngày ${h} giờ` : h > 0 ? `${h} giờ ${m} phút` : `${m} phút`;
}

/** Mốc thời gian cách đây `secs` giây: "hôm nay 12:53" / "30/10/2026 07:57". */
function ago(secs: number): string {
  const t = new Date(Date.now() - secs * 1000);
  const hm = t.toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit" });
  return t.toDateString() === new Date().toDateString() ? `hôm nay ${hm}` : `${t.toLocaleDateString("vi-VN")} ${hm}`;
}

const join = (parts: (string | null | undefined | false)[], sep = " · ") => parts.filter(Boolean).join(sep);

function slotText(s: MemorySlot): string {
  if (s.sizeMb == null) return "trống";
  const speed =
    s.speedMts && s.configuredMts && s.configuredMts !== s.speedMts
      ? `${s.configuredMts} MT/s (thanh hỗ trợ ${s.speedMts})`
      : s.configuredMts || s.speedMts
        ? `${s.configuredMts ?? s.speedMts} MT/s`
        : null;
  return join([formatBytes(s.sizeMb * 1024 * 1024, 0), s.kind, speed, s.formFactor, s.manufacturer, s.partNumber]);
}

const filled = (i: SystemInfo) => i.memory.slots.filter((s) => s.sizeMb != null).length;

/** Văn bản gọn để dán gửi người khác (không có số serial / tên máy — app không thu thập). */
export function systemInfoText(i: SystemInfo): string {
  const w = i.windows;
  const c = i.cpu;
  const lines = [
    "Cấu hình máy (PC Manager)",
    `Windows: ${join([w.product, w.displayVersion, w.build && `build ${w.build}`, w.architecture], ", ")}`,
    `CPU: ${join([c.name, c.cores && c.threads ? `${c.cores} nhân / ${c.threads} luồng` : null, ghz(c.baseMhz), c.maxMhz ? `tối đa ${ghz(c.maxMhz)}` : null], ", ")}`,
    ...(i.gpus.length
      ? i.gpus.map((g) => `GPU: ${join([g.name, g.vramBytes ? formatBytes(g.vramBytes, 0) : null, g.driverVersion && `driver ${g.driverVersion}`], ", ")}`)
      : ["GPU: —"]),
    `Mainboard: ${join([i.board.manufacturer, i.board.product, i.board.version], " ") || "—"}`,
    i.board.systemManufacturer && i.board.systemManufacturer !== i.board.manufacturer
      ? `Máy: ${join([i.board.systemManufacturer, i.board.systemProduct], " ")}`
      : null,
    `BIOS: ${join([i.bios.vendor, i.bios.version, i.bios.releaseDate && `(${i.bios.releaseDate})`], " ") || "—"}`,
    `RAM: ${formatBytes(i.memory.totalBytes, 0)}${i.memory.slotsTotal ? ` — ${filled(i)}/${i.memory.slotsTotal} khe` : ""}`,
    ...i.memory.slots.map((s) => `  - ${s.locator ?? "Khe"}: ${slotText(s)}`),
    "Ổ đĩa:",
    ...i.disks.map((d) => `  - ${d.model}: ${join([diskSize(d.sizeBytes), d.bus, d.media], ", ")}`),
  ];
  return lines.filter((l): l is string => l != null).join("\n");
}

async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Dự phòng khi Clipboard API bị chặn.
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    if (!ok) throw new Error("không sao chép được vào clipboard");
  }
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-3 py-1.5 text-sm">
      <div className="w-40 shrink-0 opacity-60">{label}</div>
      <div className="min-w-0 flex-1 break-words">{children ?? "—"}</div>
    </div>
  );
}

export default function SystemInfoPage() {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  const load = useCallback(() => {
    setLoading(true);
    api
      .systemInfo()
      .then((i) => {
        setInfo(i);
        setError(null);
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);
  useEffect(load, [load]);

  const copy = async () => {
    if (!info) return;
    try {
      await copyText(systemInfoText(info));
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const w = info?.windows;
  const c = info?.cpu;
  return (
    <PageShell
      title="Cấu hình máy"
      subtitle="Phần cứng và Windows — đọc trực tiếp từ firmware, ổ đĩa và registry. Không gửi đi đâu."
      actions={
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" loading={loading} onClick={load}>
            {!loading && <IconRefresh />}Tải lại
          </Button>
          <Button variant="primary" size="sm" disabled={!info} onClick={copy} title="Sao chép cấu hình dạng văn bản để gửi người khác">
            {copied && <IconCheck />}
            {copied ? "Đã sao chép" : "Sao chép cấu hình"}
          </Button>
        </div>
      }
    >
      {error && (
        <div className="mb-3">
          <Notice tone="error" onClose={() => setError(null)}>
            {error}
          </Notice>
        </div>
      )}
      {!info ? (
        <Card className="py-10">
          <ProgressBar className="mx-auto max-w-md" pct={null} label="Đang đọc cấu hình máy…" />
        </Card>
      ) : (
        <div className="grid gap-4 xl:grid-cols-2">
          <Card title="Windows">
            <Row label="Phiên bản">{join([w?.product, w?.displayVersion])}</Row>
            <Row label="Build">{w?.build}</Row>
            <Row label="Kiến trúc">{w?.architecture}</Row>
            <Row label="Ngày cài">{w?.installDateMs ? formatDate(w.installDateMs) : null}</Row>
            {/* Lần bật nguồn gần nhất (kể cả khi Fast Startup bật), không tính thức dậy từ Sleep. */}
            {w?.lastPowerOnMs != null && (
              <Row label="Bật máy gần nhất">
                {ago((Date.now() - w.lastPowerOnMs) / 1000)} ({uptime((Date.now() - w.lastPowerOnMs) / 1000)} trước)
              </Row>
            )}
          </Card>
          <Card title="CPU">
            <Row label="Tên">{c?.name}</Row>
            <Row label="Nhân / luồng">{c?.cores || c?.threads ? `${c?.cores ?? "?"} nhân / ${c?.threads ?? "?"} luồng` : null}</Row>
            <Row label="Xung">{join([ghz(c?.baseMhz ?? null) && `${ghz(c?.baseMhz ?? null)} cơ bản`, ghz(c?.maxMhz ?? null) && `tối đa ${ghz(c?.maxMhz ?? null)}`])}</Row>
            <Row label="Socket">{c?.socket}</Row>
          </Card>
          <Card title="Mainboard & BIOS">
            <Row label="Mainboard">{join([info.board.manufacturer, info.board.product], " ")}</Row>
            {info.board.systemManufacturer && info.board.systemManufacturer !== info.board.manufacturer && (
              <Row label="Máy">{join([info.board.systemManufacturer, info.board.systemProduct], " ")}</Row>
            )}
            <Row label="BIOS">{join([info.bios.vendor, info.bios.version])}</Row>
            <Row label="Ngày BIOS">{info.bios.releaseDate}</Row>
          </Card>
          <Card title={info.gpus.length > 1 ? "Card đồ hoạ" : "Card đồ hoạ (GPU)"}>
            {info.gpus.length === 0 && <p className="text-sm opacity-60">Không đọc được thông tin card đồ hoạ.</p>}
            {info.gpus.map((g, n) => (
              <div key={g.name} className={n > 0 ? "mt-2 border-t border-black/5 pt-2 dark:border-white/[0.06]" : ""}>
                <Row label="Tên">{g.name}</Row>
                <Row label="Bộ nhớ">{g.vramBytes ? formatBytes(g.vramBytes, 0) : null}</Row>
                <Row label="Driver">{join([g.driverVersion, g.driverDate])}</Row>
              </div>
            ))}
          </Card>
          <Card title={`RAM — ${formatBytes(info.memory.totalBytes, 0)}`} className="xl:col-span-2">
            {info.memory.slotsTotal != null && (
              <p className="mb-2 text-sm opacity-70">
                Đã cắm {filled(info)} / {info.memory.slotsTotal} khe
                {filled(info) < info.memory.slotsTotal && " — còn khe trống để nâng cấp"}
              </p>
            )}
            <div className="grid gap-1">
              {info.memory.slots.map((s, n) => (
                <div key={`${s.locator}-${n}`} className="flex gap-3 rounded-lg bg-black/[0.03] px-3 py-2 text-sm dark:bg-white/[0.04]">
                  <div className="w-44 shrink-0 truncate opacity-60" title={join([s.locator, s.bank])}>
                    {s.locator ?? `Khe ${n + 1}`}
                  </div>
                  <div className={`min-w-0 flex-1 ${s.sizeMb == null ? "opacity-50" : ""}`}>{slotText(s)}</div>
                </div>
              ))}
              {info.memory.slots.length === 0 && <p className="text-sm opacity-60">Firmware không cung cấp thông tin từng thanh RAM.</p>}
            </div>
          </Card>
          <Card title="Ổ đĩa" className="xl:col-span-2">
            <div className="grid gap-1">
              {info.disks.map((d) => (
                <div key={d.index} className="flex flex-wrap items-baseline gap-x-3 rounded-lg bg-black/[0.03] px-3 py-2 text-sm dark:bg-white/[0.04]">
                  <div className="min-w-0 flex-1 font-medium">{d.model}</div>
                  <div className="tabular-nums">{diskSize(d.sizeBytes)}</div>
                  <div className="w-28 text-right opacity-70">{join([d.bus, d.media], " ") || "—"}</div>
                </div>
              ))}
            </div>
          </Card>
        </div>
      )}
    </PageShell>
  );
}
