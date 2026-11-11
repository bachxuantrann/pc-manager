import { useEffect, useMemo, useRef, useState } from "react";
import Button from "../components/Button";
import { Card, Notice } from "../components/Card";
import ConfirmDialog, { type ConfirmRequest } from "../components/ConfirmDialog";
import PageShell from "../components/PageShell";
import ProgressBar from "../components/ProgressBar";
import type { PageId } from "../components/Sidebar";
import { AdminNotice, useElevation } from "../components/Elevation";
import { useJobResume } from "../hooks/useJobResume";
import { useNavigate } from "../hooks/useNavigate";
import { useReportBusy } from "../hooks/usePageActive";
import {
  api,
  errorMessage,
  type DiagCategory,
  type DiagFinding,
  type DiagProgress,
  type DiagReport,
  type DiagSeverity,
} from "../lib/api";
import { formatBytes } from "../lib/format";

const CATEGORY_LABEL: Record<DiagCategory, string> = {
  memory: "Bộ nhớ (RAM)",
  storage: "Ổ đĩa & rác",
  startup: "Khởi động",
  apps: "Ứng dụng",
  windows: "Windows",
  hardware: "Phần cứng",
};

const SEVERITY: Record<DiagSeverity, { label: string; badge: string; icon: string; rank: number }> = {
  ok: { label: "Tốt", badge: "bg-green-500/15 text-green-700 dark:text-green-400", icon: "✓", rank: 0 },
  info: { label: "Gợi ý", badge: "bg-sky-500/15 text-sky-700 dark:text-sky-300", icon: "ⓘ", rank: 1 },
  warn: { label: "Cần chú ý", badge: "bg-amber-500/15 text-amber-700 dark:text-amber-300", icon: "⚠", rank: 2 },
  critical: { label: "Nên xử lý", badge: "bg-red-500/15 text-red-700 dark:text-red-400", icon: "⚠", rank: 3 },
};

function Badge({ s }: { s: DiagSeverity }) {
  return <span className={`shrink-0 rounded-full px-2.5 py-0.5 text-xs font-medium ${SEVERITY[s].badge}`}>{SEVERITY[s].label}</span>;
}

function headline(r: DiagReport): { text: string; tone: string } {
  const issues = r.findings.filter((f) => f.severity === "warn" || f.severity === "critical").length;
  const hints = r.findings.filter((f) => f.severity === "info").length;
  if (r.findings.some((f) => f.severity === "critical")) return { text: `${issues} vấn đề nên xử lý`, tone: "text-red-600 dark:text-red-400" };
  if (issues > 0) return { text: `${issues} điểm cần chú ý`, tone: "text-amber-600 dark:text-amber-300" };
  if (hints > 0) return { text: `Máy ổn · ${hints} gợi ý`, tone: "text-green-600 dark:text-green-400" };
  return { text: "Máy đang ổn", tone: "text-green-600 dark:text-green-400" };
}

/** Verify: so với lần quét trước — vấn đề nào đã hết hoặc nhẹ đi. */
function improvements(prev: DiagReport, next: DiagReport): string[] {
  const rank = (s: DiagSeverity) => SEVERITY[s].rank;
  return prev.findings
    .filter((p) => p.severity !== "ok")
    .flatMap((p) => {
      const n = next.findings.find((f) => f.id === p.id && f.title.split(":")[0] === p.title.split(":")[0]) ?? next.findings.find((f) => f.id === p.id);
      if (n && rank(n.severity) >= rank(p.severity)) return [];
      const change = p.metric && n?.metric && p.metric !== n.metric ? `: ${p.metric} → ${n.metric}` : n ? "" : ": đã hết";
      return [`${p.title}${change}`];
    });
}

export default function Diagnosis() {
  const { elevated } = useElevation();
  const navigate = useNavigate();
  const [report, setReport] = useState<DiagReport | null>(null);
  const [prev, setPrev] = useState<DiagReport | null>(null);
  const [scannedAt, setScannedAt] = useState<number | null>(null);
  const [running, setRunning] = useState<null | "quick" | "deep">(null);
  const [progress, setProgress] = useState<DiagProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);
  useReportBusy(running !== null);
  const reportRef = useRef<DiagReport | null>(null);
  reportRef.current = report;

  const apply = (r: DiagReport) => {
    setPrev(reportRef.current);
    setReport(r);
    setScannedAt(Date.now());
  };

  const scan = async (deep: boolean) => {
    setRunning(deep ? "deep" : "quick");
    setProgress(null);
    setError(null);
    setMsg(null);
    try {
      apply(await api.diagnosisRun(deep, setProgress));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setRunning(null);
      setProgress(null);
    }
  };

  // Mở trang lần đầu => tự quét nhanh (chỉ đọc, ~0.2 giây).
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void scan(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Cửa sổ bị đóng (xuống tray) lúc đang quét sâu rồi mở lại: hiện lại tiến độ / kết quả.
  useJobResume<DiagProgress>("diagnosis.scan", {
    onRunning: (p) => {
      setRunning("deep");
      setProgress(p);
    },
    onProgress: setProgress,
    onDone: (o) => {
      setRunning(null);
      setProgress(null);
      if (o.ok) apply(o.value as DiagReport);
      else setError(errorMessage(o.value));
    },
  });

  const boost = (f: DiagFinding) =>
    setConfirm({
      title: "Boost bộ nhớ?",
      body: (
        <p>
          Giải phóng cache ít dùng trong RAM (chế độ nhẹ — giữ cache của app hay dùng). Không đóng app nào, không mất dữ liệu.
          {f.metric && <span className="mt-1 block text-xs opacity-70">Hiện tại: {f.metric}</span>}
        </p>
      ),
      confirmLabel: "Boost",
      onConfirm: async () => {
        try {
          const r = await api.memoryBoost("light");
          setMsg(`Đã Boost: RAM trống tăng ${formatBytes(Math.max(0, r.gain.availableGain))}. Đang kiểm tra lại…`);
          await scan(false);
        } catch (e) {
          setError(errorMessage(e));
        }
      },
    });

  const act = (f: DiagFinding) => {
    if (!f.action) return;
    if (f.action.kind === "boost") boost(f);
    else navigate(f.action.page as PageId);
  };

  const issues = useMemo(() => (report?.findings ?? []).filter((f) => f.severity !== "ok"), [report]);
  const improved = useMemo(() => (prev && report ? improvements(prev, report) : []), [prev, report]);
  const h = report ? headline(report) : null;
  const pct = progress && progress.total > 0 ? (progress.done / progress.total) * 100 : null;

  return (
    <PageShell
      title="Chẩn đoán"
      subtitle="Kiểm tra RAM, ổ đĩa, rác, app khởi động, app lâu không dùng, Windows và phần cứng trong một lần quét."
      actions={
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            disabled={running !== null || elevated !== true}
            loading={running === "deep"}
            onClick={() => scan(true)}
            title={elevated === true ? "Thêm phân tích kho WinSxS bằng DISM (~1 phút)" : "Quét sâu cần quyền Administrator"}
          >
            Quét sâu
          </Button>
          <Button variant="primary" size="sm" loading={running === "quick"} disabled={running !== null} onClick={() => scan(false)}>
            {report ? "Quét lại" : "Quét máy"}
          </Button>
        </div>
      }
    >
      {elevated === false && (
        <div className="mb-3">
          <AdminNotice>Quét sâu (phân tích kho WinSxS bằng DISM) cần quyền Administrator.</AdminNotice>
        </div>
      )}
      {error && (
        <div className="mb-3">
          <Notice tone="error" onClose={() => setError(null)}>
            {error}
          </Notice>
        </div>
      )}
      {msg && !running && (
        <div className="mb-3">
          <Notice tone="ok" onClose={() => setMsg(null)}>
            {msg}
          </Notice>
        </div>
      )}
      {improved.length > 0 && !running && (
        <div className="mb-3">
          <Notice tone="ok" onClose={() => setPrev(null)}>
            <b>Đã cải thiện so với lần quét trước:</b>
            <ul className="mt-1 list-disc pl-5">
              {improved.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          </Notice>
        </div>
      )}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.6fr)]">
        <Card title="Tình trạng máy">
          {running ? (
            <ProgressBar
              className="py-2"
              pct={pct}
              label={progress ? `Đang kiểm tra: ${progress.step}` : running === "deep" ? "Đang bắt đầu quét sâu…" : "Đang quét…"}
            />
          ) : (
            h && (
              <div>
                <div className={`text-2xl font-semibold ${h.tone}`}>{h.text}</div>
                {report && scannedAt && (
                  <div className="mt-1 text-xs opacity-60">
                    {report.deep ? "Quét sâu" : "Quét nhanh"} lúc {new Date(scannedAt).toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit" })} ·{" "}
                    {(report.durationMs / 1000).toFixed(1)} giây · chỉ đọc, không thay đổi gì
                  </div>
                )}
              </div>
            )
          )}
          {report && (
            <div className="mt-4 grid gap-1">
              {report.categories.map((c) => (
                <div key={c.category} className="flex items-center justify-between gap-3 rounded-lg px-1 py-1.5 text-sm">
                  <span>{CATEGORY_LABEL[c.category]}</span>
                  <Badge s={c.severity} />
                </div>
              ))}
            </div>
          )}
          {report && report.notes.length > 0 && (
            <ul className="mt-3 grid gap-1 text-xs opacity-60">
              {report.notes.map((n) => (
                <li key={n}>• {n}</li>
              ))}
            </ul>
          )}
        </Card>

        <Card title={issues.length ? "Đề xuất" : "Đề xuất"}>
          {!report && !running && <p className="text-sm opacity-60">Bấm "Quét máy" để kiểm tra.</p>}
          {report && issues.length === 0 && <p className="text-sm opacity-70">Không có gì cần xử lý. 🎉</p>}
          <div className="grid gap-2">
            {issues.map((f) => (
              <div key={`${f.id}-${f.title}`} className="flex gap-3 rounded-xl border border-black/5 p-3 dark:border-white/[0.06]">
                <span
                  className={`mt-0.5 w-4 shrink-0 text-center ${f.severity === "critical" ? "text-red-500" : f.severity === "warn" ? "text-amber-500" : "text-sky-500"}`}
                  aria-hidden
                >
                  {SEVERITY[f.severity].icon}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium">{f.title}</span>
                    <Badge s={f.severity} />
                  </div>
                  <p className="mt-0.5 text-sm opacity-75">{f.detail}</p>
                  {f.metric && <p className="mt-0.5 text-xs opacity-60">{f.metric}</p>}
                </div>
                {f.action && (
                  <Button
                    variant={f.severity === "critical" || f.severity === "warn" ? "primary" : "secondary"}
                    size="sm"
                    className="self-center"
                    disabled={running !== null}
                    onClick={() => act(f)}
                  >
                    {f.action.kind === "boost" ? "Boost" : f.action.label}
                  </Button>
                )}
              </div>
            ))}
          </div>
          {issues.some((f) => f.action?.kind === "open") && (
            <p className="mt-3 text-xs opacity-60">
              Các nút mở đúng trang để bạn xem trước và tự xác nhận như bình thường. Xử lý xong, quay lại bấm "Quét lại" để kiểm tra kết quả.
            </p>
          )}
        </Card>
      </div>
      <ConfirmDialog req={confirm} onClose={() => setConfirm(null)} />
    </PageShell>
  );
}
