import { useMemo, useState } from "react";
import Button from "../components/Button";
import { Card, Notice } from "../components/Card";
import ConfirmDialog, { type ConfirmRequest } from "../components/ConfirmDialog";
import PageShell from "../components/PageShell";
import SystemDeepClean from "../components/SystemDeepClean";
import { AdminNotice } from "../components/Elevation";
import { api, errorMessage, type CleanerProgress, type CleanReport, type Risk, type RuleScan, type ScanResult } from "../lib/api";
import { formatBytes } from "../lib/format";

type Phase = "idle" | "scanning" | "preview" | "cleaning" | "report";

const RISK: Record<Risk, { label: string; cls: string; hint: string }> = {
  low: { label: "An toàn", cls: "bg-green-500/15 text-green-700 dark:text-green-300", hint: "Windows/ứng dụng tự tạo lại khi cần" },
  medium: { label: "Cân nhắc", cls: "bg-amber-500/15 text-amber-700 dark:text-amber-300", hint: "Có thể cần khi điều tra lỗi" },
  high: { label: "Không khôi phục được", cls: "bg-red-500/15 text-red-700 dark:text-red-300", hint: "Dữ liệu bạn đã xoá" },
};

const isReady = (r: RuleScan) => r.status.kind === "ready";

function statusNote(r: RuleScan): string | null {
  switch (r.status.kind) {
    case "empty":
      return r.skippedYoung > 0 ? `Chỉ có file mới (${r.skippedYoung}) — được giữ lại` : "Không có gì để dọn";
    case "needsAdmin":
      return "Cần chạy PC Manager với quyền Administrator";
    case "blocked":
      return r.status.reason;
    case "error":
      return `Lỗi: ${r.status.message}`;
    default:
      return null;
  }
}

function ProgressView({ p, onCancel, cancelling }: { p: CleanerProgress | null; onCancel: () => void; cancelling: boolean }) {
  const pct = p && p.totalRules > 0 ? Math.round((p.doneRules / p.totalRules) * 100) : 0;
  const verb = p?.phase === "clean" ? "Đang dọn" : "Đang quét";
  return (
    <Card className="flex flex-col gap-4">
      <div className="flex items-baseline justify-between gap-4">
        <div className="min-w-0">
          <div className="text-lg font-semibold">
            {verb}… {pct}%
          </div>
          <div className="truncate text-sm opacity-70">{p?.current ?? "Chuẩn bị…"}</div>
        </div>
        <Button variant="secondary" onClick={onCancel} loading={cancelling}>
          {cancelling ? "Đang huỷ…" : "Huỷ"}
        </Button>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-black/10 dark:bg-white/10">
        <div className="h-full rounded-full bg-accent transition-[width] duration-300" style={{ width: `${pct}%` }} />
      </div>
      <div className="text-sm tabular-nums opacity-70">
        {p ? `${p.files.toLocaleString("vi-VN")} file · ${formatBytes(p.bytes)}` : "…"}
      </div>
    </Card>
  );
}

function RuleRow({ r, checked, onToggle }: { r: RuleScan; checked: boolean; onToggle: () => void }) {
  const ready = isReady(r);
  const note = statusNote(r);
  const risk = RISK[r.risk];
  return (
    <label
      className={`flex items-start gap-3 border-t border-black/5 px-4 py-3 first:border-t-0 dark:border-white/5 ${
        ready ? "cursor-pointer hover:bg-black/[0.03] dark:hover:bg-white/[0.03]" : "cursor-not-allowed opacity-55"
      }`}
    >
      <input type="checkbox" className="mt-1 h-4 w-4 shrink-0 accent-[var(--color-accent)]" checked={checked} disabled={!ready} onChange={onToggle} />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium">{r.name}</span>
          <span className={`rounded px-1.5 py-0.5 text-[11px] font-medium ${risk.cls}`} title={risk.hint}>
            {risk.label}
          </span>
        </div>
        <p className="mt-0.5 text-xs leading-relaxed opacity-65">{r.description}</p>
        {note && <p className="mt-1 text-xs text-amber-700 dark:text-amber-300">{note}</p>}
        {ready && r.skippedYoung > 0 && (
          <p className="mt-1 text-xs opacity-60">Giữ lại {r.skippedYoung.toLocaleString("vi-VN")} file mới tạo gần đây</p>
        )}
      </div>
      <div className="shrink-0 text-right">
        <div className="font-semibold tabular-nums">{ready ? formatBytes(r.bytes) : "—"}</div>
        {ready && <div className="text-xs tabular-nums opacity-60">{r.files.toLocaleString("vi-VN")} mục</div>}
      </div>
    </label>
  );
}

function ReportView({ rep, onRescan, onRealClean }: { rep: CleanReport; onRescan: () => void; onRealClean?: () => void }) {
  return (
    <div className="flex flex-col gap-4">
      <Card>
        {rep.dryRun && <div className="mb-3"><Notice tone="warn">Chế độ chạy thử — chưa có file nào bị xoá.</Notice></div>}
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <div className="text-sm opacity-70">{rep.dryRun ? "Sẽ giải phóng" : "Đã giải phóng"}</div>
            <div className="text-4xl font-semibold text-green-600 tabular-nums dark:text-green-400">{formatBytes(rep.freedBytes)}</div>
            <div className="mt-1 text-sm opacity-70">
              {rep.deletedFiles.toLocaleString("vi-VN")} mục · {(rep.durationMs / 1000).toFixed(1)} giây
              {rep.cancelled && " · đã huỷ giữa chừng"}
            </div>
          </div>
          <div className="flex gap-2">
            {onRealClean && (
              <Button variant="primary" onClick={onRealClean}>
                Dọn thật
              </Button>
            )}
            <Button variant="secondary" onClick={onRescan}>
              Quét lại
            </Button>
          </div>
        </div>
        {(rep.skippedLocked > 0 || rep.skippedChanged > 0 || rep.errors > 0) && (
          <p className="mt-3 text-sm opacity-75">
            Bỏ qua: {rep.skippedLocked > 0 && `${rep.skippedLocked} file đang được dùng`}
            {rep.skippedLocked > 0 && rep.skippedChanged > 0 && " · "}
            {rep.skippedChanged > 0 && `${rep.skippedChanged} file đã thay đổi từ lúc quét`}
            {rep.errors > 0 && ` · ${rep.errors} lỗi`}. Đây là hành vi an toàn: PC Manager không ép xoá file đang dùng.
          </p>
        )}
      </Card>
      <Card className="overflow-hidden !p-0">
        <div className="px-4 pt-3 pb-2 text-sm font-medium opacity-70">Chi tiết</div>
        <table className="w-full text-sm">
          <tbody>
            {rep.rules.map((r) => (
              <tr key={r.id} className="border-t border-black/5 dark:border-white/5">
                <td className="px-4 py-2">
                  {r.name}
                  {r.message && <div className="text-xs text-amber-700 dark:text-amber-300">{r.message}</div>}
                  {r.firstError && <div className="truncate text-xs text-red-600 dark:text-red-400">{r.firstError}</div>}
                </td>
                <td className="px-4 py-2 text-right tabular-nums">{formatBytes(r.freedBytes)}</td>
                <td className="px-4 py-2 text-right text-xs tabular-nums opacity-70">
                  {r.deletedFiles.toLocaleString("vi-VN")} mục
                  {r.skippedLocked + r.skippedChanged > 0 && ` · bỏ qua ${(r.skippedLocked + r.skippedChanged).toLocaleString("vi-VN")}`}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
    </div>
  );
}

export default function Cleaner() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<CleanerProgress | null>(null);
  const [report, setReport] = useState<CleanReport | null>(null);
  const [dryRun, setDryRun] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);

  const startScan = async () => {
    setPhase("scanning");
    setError(null);
    setProgress(null);
    setReport(null);
    setCancelling(false);
    try {
      const r = await api.cleanerScan(setProgress);
      setScan(r);
      setSelected(new Set(r.rules.filter((x) => isReady(x) && x.defaultSelected).map((x) => x.id)));
      setPhase(r.cancelled ? "idle" : "preview");
      if (r.cancelled) setError("Đã huỷ quét.");
    } catch (e) {
      setError(errorMessage(e));
      setPhase("idle");
    }
  };

  const runClean = async (dry: boolean) => {
    if (!scan) return;
    setPhase("cleaning");
    setError(null);
    setProgress(null);
    setCancelling(false);
    try {
      const ids = scan.rules.filter((r) => selected.has(r.id)).map((r) => r.id);
      setReport(await api.cleanerClean(scan.scanId, ids, dry, setProgress));
      setPhase("report");
    } catch (e) {
      setError(errorMessage(e));
      setPhase("preview");
    }
  };

  const cancel = () => {
    setCancelling(true);
    api.cleanerCancel().catch(() => {});
  };

  const groups = useMemo(() => {
    const m = new Map<string, RuleScan[]>();
    for (const r of scan?.rules ?? []) m.set(r.group, [...(m.get(r.group) ?? []), r]);
    return [...m.entries()];
  }, [scan]);

  const chosen = (scan?.rules ?? []).filter((r) => selected.has(r.id) && isReady(r));
  const chosenBytes = chosen.reduce((s, r) => s + r.bytes, 0);

  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const toggleGroup = (rules: RuleScan[], on: boolean) =>
    setSelected((s) => {
      const n = new Set(s);
      for (const r of rules.filter(isReady)) {
        if (on) n.add(r.id);
        else n.delete(r.id);
      }
      return n;
    });

  const askClean = () => {
    if (dryRun) return void runClean(true);
    const warns = [
      chosen.some((r) => r.risk === "high") && "Thùng rác sẽ bị làm rỗng — không khôi phục được.",
      chosen.some((r) => r.usesService) && "Dịch vụ Windows Update sẽ tạm dừng trong lúc dọn rồi tự chạy lại.",
    ].filter(Boolean) as string[];
    setConfirm({
      title: `Dọn ${formatBytes(chosenBytes)}?`,
      body: (
        <div className="space-y-2">
          <p>
            {chosen.length} mục: {chosen.map((r) => r.name).join(", ")}.
          </p>
          <p className="text-xs opacity-70">File đang được dùng, file mới tạo và file thay đổi từ lúc quét sẽ được bỏ qua.</p>
          {warns.map((w) => (
            <p key={w} className="text-amber-700 dark:text-amber-300">
              {w}
            </p>
          ))}
        </div>
      ),
      confirmLabel: "Dọn",
      danger: chosen.some((r) => r.risk === "high"),
      onConfirm: () => runClean(false),
    });
  };

  return (
    <PageShell
      title="Dọn rác"
      subtitle="Quét → xem trước → bạn chọn → dọn. Chỉ xoá những gì có trong quy tắc dọn rác, không đụng tới dữ liệu cá nhân."
      actions={
        (phase === "idle" || phase === "preview") && (
          <Button variant={phase === "idle" ? "primary" : "secondary"} onClick={startScan}>
            {phase === "idle" ? "Quét ngay" : "Quét lại"}
          </Button>
        )
      }
    >
      {error && (
        <div className="mb-4">
          <Notice tone="error">{error}</Notice>
        </div>
      )}
      <div className="mb-4 empty:hidden">
        <AdminNotice>Windows\Temp, crash dump hệ thống, Windows Update cache… chỉ dọn được với quyền Administrator.</AdminNotice>
      </div>

      {phase === "idle" && (
        <Card className="flex flex-col items-center gap-3 py-12 text-center">
          <div className="text-4xl">✦</div>
          <p className="max-w-lg text-sm opacity-70">
            Quét file tạm, cache trình duyệt, bản ghi lỗi, bản cập nhật Windows đã cài và Thùng rác. Không có gì bị xoá cho tới khi bạn
            xem trước và xác nhận.
          </p>
          <Button variant="primary" onClick={startScan}>
            Quét ngay
          </Button>
        </Card>
      )}

      {(phase === "scanning" || phase === "cleaning") && <ProgressView p={progress} onCancel={cancel} cancelling={cancelling} />}

      {phase === "report" && report && (
        <ReportView rep={report} onRescan={startScan} onRealClean={report.dryRun ? () => runClean(false) : undefined} />
      )}

      {phase === "preview" && scan && (
        <>
          {/* auto-rows-max: hàng cao đúng bằng nội dung, không bị ép dẹt khi
              khung thấp (thẻ có overflow-hidden nên min-height = 0). */}
          <div className="grid min-h-0 flex-1 auto-rows-max content-start items-start gap-4 overflow-auto pb-4 xl:grid-cols-2">
            {groups.map(([group, rules]) => {
              const ready = rules.filter(isReady);
              const on = ready.filter((r) => selected.has(r.id)).length;
              const bytes = ready.reduce((s, r) => s + r.bytes, 0);
              return (
                <Card key={group} className="overflow-hidden !p-0">
                  <label className={`flex items-center gap-3 bg-black/[0.03] px-4 py-2.5 dark:bg-white/[0.04] ${ready.length ? "cursor-pointer" : ""}`}>
                    <input
                      type="checkbox"
                      className="h-4 w-4 accent-[var(--color-accent)]"
                      disabled={!ready.length}
                      checked={ready.length > 0 && on === ready.length}
                      ref={(el) => {
                        if (el) el.indeterminate = on > 0 && on < ready.length;
                      }}
                      onChange={(e) => toggleGroup(rules, e.target.checked)}
                    />
                    <span className="flex-1 font-semibold">{group}</span>
                    <span className="text-sm tabular-nums opacity-70">{formatBytes(bytes)}</span>
                  </label>
                  {rules.map((r) => (
                    <RuleRow key={r.id} r={r} checked={selected.has(r.id)} onToggle={() => toggle(r.id)} />
                  ))}
                </Card>
              );
            })}
          </div>

          {/* Thanh hành động luôn hiện ở đáy */}
          <div className="flex flex-wrap items-center gap-4 rounded-lg bg-white px-4 py-3 shadow-sm dark:bg-white/5">
            <div className="min-w-0 flex-1">
              <div className="text-lg font-semibold tabular-nums">{formatBytes(chosenBytes)}</div>
              <div className="text-xs opacity-60">
                {chosen.length} mục đã chọn · quét trong {(scan.durationMs / 1000).toFixed(1)} giây
              </div>
            </div>
            <label className="flex cursor-pointer items-center gap-2 text-sm" title="Chạy toàn bộ kiểm tra nhưng không xoá gì">
              <input type="checkbox" checked={dryRun} onChange={(e) => setDryRun(e.target.checked)} className="accent-[var(--color-accent)]" />
              Chạy thử (không xoá)
            </label>
            <Button variant="primary" disabled={chosen.length === 0} onClick={askClean}>
              {dryRun ? "Chạy thử" : `Dọn ${formatBytes(chosenBytes)}`}
            </Button>
          </div>
        </>
      )}

      <ConfirmDialog req={confirm} onClose={() => setConfirm(null)} />
      <SystemDeepClean />
    </PageShell>
  );
}
