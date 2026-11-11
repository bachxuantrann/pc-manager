import { PANEL } from "./ui";
import { useEffect, useState } from "react";
import Button, { Spinner } from "./Button";
import { Card, Notice } from "./Card";
import ConfirmDialog, { type ConfirmRequest } from "./ConfirmDialog";
import { AdminNotice, useElevation } from "./Elevation";
import { api, errorMessage, type SystemCleanReport, type WinSxsAnalysis, type WindowsOldInfo } from "../lib/api";
import { formatBytes } from "../lib/format";
import { useReportBusy } from "../hooks/usePageActive";
import { useJobResume } from "../hooks/useJobResume";
import ProgressBar from "./ProgressBar";

type Busy = null | "analyze" | "winsxs" | "winold";

/** Dưới mức này coi như không lấy lại được gì (chương trình khác vẫn ghi/xoá file trong lúc dọn). */
const NOTHING_FREED = 16 * 1024 * 1024;

function freedText(r: SystemCleanReport, what: "winsxs" | "winold"): string {
  const secs = Math.max(1, Math.round(r.durationMs / 1000));
  const reboot = r.rebootRequired ? " Cần khởi động lại máy để hoàn tất." : "";
  if (r.freedBytes >= NOTHING_FREED) return `Xong sau ${secs} giây — giải phóng khoảng ${formatBytes(r.freedBytes)}.${reboot}`;
  if (what === "winsxs") {
    // DISM trả "thành công" ngay cả khi không còn gì để dọn (vd. vừa dọn xong lần trước).
    return (
      `DISM báo thành công sau ${secs} giây nhưng không còn gì để dọn thêm — kho đã được dọn gần đây.${reboot} ` +
      "Nếu bấm Phân tích vẫn thấy còn gói có thể thu hồi, thường phải khởi động lại máy rồi dọn lại."
    );
  }
  return `Xong sau ${secs} giây — không thấy dung lượng trống tăng thêm.${reboot}`;
}

/** Kết quả DISM /AnalyzeComponentStore: tách phần không dọn được khỏi phần dọn được. */
function AnalysisView({ a }: { a: WinSxsAnalysis }) {
  if (a.actualSize == null) {
    // DISM không trả về định dạng mong đợi — hiện nguyên văn.
    return <pre className="mt-1 whitespace-pre-wrap text-xs opacity-70">{a.raw}</pre>;
  }
  const row = (label: string, value: string | null, note: string, tone = "") =>
    value != null && (
      <div className="flex items-baseline gap-2">
        <span className="w-48 shrink-0 opacity-70">{label}</span>
        <b className={`w-20 shrink-0 text-right tabular-nums ${tone}`}>{value}</b>
        <span className="opacity-60">{note}</span>
      </div>
    );
  return (
    <div className="mt-2 grid gap-0.5 text-xs">
      {row("Tổng kho WinSxS", a.actualSize, "")}
      {row("├ Dùng chung với Windows", a.sharedWithWindows, "chính là file Windows đang dùng — không dọn được", "opacity-60")}
      {row("├ Bản sao lưu, tính năng tắt", a.backups, "dọn được một phần (bản cũ đã bị thay thế)")}
      {row("└ Cache, dữ liệu tạm", a.cache, "dọn được")}
      <div className="mt-1">
        {a.recommended ? (
          <b className="text-amber-700 dark:text-amber-300">DISM khuyên nên dọn</b>
        ) : (
          <span className="text-green-700 dark:text-green-400">Chưa cần dọn</span>
        )}
        {a.reclaimablePackages != null && ` · ${a.reclaimablePackages} gói có thể thu hồi`}
        {a.lastCleanup && <span className="opacity-60"> · lần dọn gần nhất: {a.lastCleanup}</span>}
      </div>
    </div>
  );
}

/**
 * Dọn sâu hệ thống bằng công cụ chính thức của Windows:
 * WinSxS qua DISM, Windows.old qua Disk Cleanup. Cần quyền Administrator.
 */
export default function SystemDeepClean() {
  const { elevated } = useElevation();
  const [busy, setBusy] = useState<Busy>(null);
  useReportBusy(busy !== null);
  const [progress, setProgress] = useState<number | null>(null);
  const [analysis, setAnalysis] = useState<WinSxsAnalysis | null>(null);
  const [winOld, setWinOld] = useState<WindowsOldInfo | null>(null);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);

  useEffect(() => {
    api.systemWindowsOldInfo().then(setWinOld).catch(() => setWinOld(null));
  }, []);

  const locked = elevated !== true || busy !== null;

  const analyze = async () => {
    setBusy("analyze");
    setProgress(0);
    // Giữ thông báo kết quả dọn (nếu có); chỉ xoá thông báo lỗi cũ.
    setMsg((m) => (m?.tone === "error" ? null : m));
    try {
      setAnalysis(await api.systemAnalyzeWinsxs(setProgress));
    } catch (e) {
      setMsg({ tone: "error", text: errorMessage(e) });
    } finally {
      setBusy(null);
      setProgress(null);
    }
  };

  const cleanWinsxs = () =>
    setConfirm({
      title: "Dọn WinSxS bằng DISM?",
      body: (
        <div className="space-y-2">
          <p>
            Chạy <code>DISM /StartComponentCleanup</code> — xoá các phiên bản cũ của thành phần Windows đã được thay bằng bản cập nhật
            mới. Thường mất 5–20 phút, <b>không dừng giữa chừng được</b>.
          </p>
          <p>
            Thường chỉ lấy lại vài trăm MB tới vài GB: phần lớn kho WinSxS là file Windows đang dùng (mục "Dùng chung với Windows") và
            không bao giờ dọn được.
          </p>
          <p className="text-xs opacity-70">Không dùng /ResetBase: các bản cập nhật đã cài vẫn gỡ được như bình thường.</p>
        </div>
      ),
      confirmLabel: "Bắt đầu dọn",
      onConfirm: async () => {
        setBusy("winsxs");
        setProgress(0);
        setMsg(null);
        try {
          afterWinsxsClean(await api.systemCleanWinsxs(setProgress));
        } catch (e) {
          setMsg({ tone: "error", text: `WinSxS: ${errorMessage(e)}` });
        } finally {
          setBusy(null);
          setProgress(null);
        }
      },
    });

  /**
   * Dọn xong: báo kết quả, ẩn bảng phân tích cũ (đã sai sau khi dọn). Không tự phân tích lại
   * (mất ~1 phút) — người dùng tự bấm "Phân tích" khi muốn xem kho sau khi dọn.
   */
  const afterWinsxsClean = (r: SystemCleanReport) => {
    setMsg({ tone: "ok", text: `WinSxS: ${freedText(r, "winsxs")}` });
    setAnalysis(null);
    setProgress(null);
  };

  // Cửa sổ bị đóng (xuống tray) lúc DISM / Disk Cleanup đang chạy rồi mở lại: hiện lại trạng thái.
  useJobResume<number>("winsxs.analyze", {
    onRunning: (p) => {
      setMsg(null);
      setBusy("analyze");
      setProgress(p ?? 0);
    },
    onProgress: setProgress,
    onDone: (o) => {
      setBusy(null);
      setProgress(null);
      if (o.ok) setAnalysis(o.value as WinSxsAnalysis);
      else setMsg({ tone: "error", text: `Phân tích WinSxS: ${errorMessage(o.value)}` });
    },
  });
  useJobResume<number>("winsxs.clean", {
    onRunning: (p) => {
      setMsg(null);
      setBusy("winsxs");
      setProgress(p ?? 0);
    },
    onProgress: setProgress,
    onDone: (o) => {
      setProgress(null);
      setBusy(null);
      if (o.ok) afterWinsxsClean(o.value as SystemCleanReport);
      else setMsg({ tone: "error", text: `WinSxS: ${errorMessage(o.value)}` });
    },
  });
  useJobResume<number>("winold.clean", {
    onRunning: (p) => {
      setMsg(null);
      setBusy("winold");
      setProgress(p ?? 0);
    },
    onProgress: setProgress,
    onDone: (o) => {
      setBusy(null);
      setProgress(null);
      setMsg(
        o.ok
          ? { tone: "ok", text: `Windows.old: ${freedText(o.value as SystemCleanReport, "winold")}` }
          : { tone: "error", text: `Windows.old: ${errorMessage(o.value)}` },
      );
      api.systemWindowsOldInfo().then(setWinOld).catch(() => {});
    },
  });

  const cleanWinOld = () =>
    setConfirm({
      title: "Xoá bản cài Windows trước (Windows.old)?",
      body: (
        <div className="space-y-2">
          <p>
            Sau khi xoá, bạn <b>không thể quay lại phiên bản Windows trước</b> (Settings → Recovery → Go back). File cá nhân còn sót trong
            Windows.old cũng bị xoá — hãy kiểm tra trước nếu cần.
          </p>
          <p className="text-xs opacity-70">PC Manager dùng Disk Cleanup của Windows (chỉ bật mục "Previous Windows installation").</p>
        </div>
      ),
      confirmLabel: "Xoá Windows.old",
      danger: true,
      onConfirm: async () => {
        setBusy("winold");
        setProgress(0);
        setMsg(null);
        try {
          const r = await api.systemCleanWindowsOld(setProgress);
          setMsg({ tone: "ok", text: `Windows.old: ${freedText(r, "winold")}` });
          setWinOld(await api.systemWindowsOldInfo());
        } catch (e) {
          setMsg({ tone: "error", text: `Windows.old: ${errorMessage(e)}` });
        } finally {
          setBusy(null);
          setProgress(null);
        }
      },
    });

  return (
    <Card title="Dọn sâu hệ thống" className="mt-4">
      <div className="grid gap-3">
        <AdminNotice>Dọn sâu hệ thống cần quyền Administrator.</AdminNotice>

        <div className={`flex flex-wrap items-start justify-between gap-3 p-3 ${PANEL}`}>
          <div className="min-w-0 flex-1 text-sm">
            <div className="font-medium">Kho thành phần Windows (WinSxS)</div>
            <div className="text-xs opacity-70">
              Dọn bằng DISM — cách duy nhất an toàn. Không bao giờ xoá trực tiếp file trong WinSxS.
            </div>
            {analysis && <AnalysisView a={analysis} />}
            {busy === "winsxs" && <ProgressBar className="mt-2" pct={progress} label="Đang dọn bằng DISM — đừng tắt máy." />}
            {busy === "analyze" && <ProgressBar className="mt-2" pct={progress} label="Đang phân tích kho WinSxS (chỉ đọc)…" />}
          </div>
          <div className="flex gap-2">
            <Button variant="secondary" size="sm" disabled={locked} loading={busy === "analyze"} onClick={analyze} title="DISM /AnalyzeComponentStore — chỉ đọc, ~1 phút">
              Phân tích
            </Button>
            <Button size="sm" disabled={locked} loading={busy === "winsxs"} onClick={cleanWinsxs}>
              Dọn WinSxS
            </Button>
          </div>
        </div>

        <div className={`flex flex-wrap items-start justify-between gap-3 p-3 ${PANEL}`}>
          <div className="min-w-0 flex-1 text-sm">
            <div className="font-medium">Bản cài Windows trước (Windows.old)</div>
            <div className="text-xs opacity-70">
              {winOld == null ? (
                <span className="inline-flex items-center gap-1">
                  <Spinner className="h-3 w-3" /> Đang kiểm tra…
                </span>
              ) : winOld.exists ? (
                <>
                  {winOld.path} · khoảng <b>{formatBytes(winOld.bytes)}</b>. Tạo ra sau khi nâng cấp Windows, dùng để quay lại bản cũ.
                </>
              ) : (
                "Không có — máy chưa nâng cấp Windows gần đây hoặc đã được dọn."
              )}
            </div>
            {busy === "winold" && (
              <ProgressBar className="mt-2" pct={progress} label="Disk Cleanup đang xoá Windows.old — % ước lượng theo dung lượng đã lấy lại." />
            )}
          </div>
          <Button variant="danger" size="sm" disabled={locked || !winOld?.exists} loading={busy === "winold"} onClick={cleanWinOld}>
            Xoá Windows.old
          </Button>
        </div>

        {msg && (
          <Notice tone={msg.tone} onClose={() => setMsg(null)}>
            {msg.text}
          </Notice>
        )}
      </div>
      <ConfirmDialog req={confirm} onClose={() => setConfirm(null)} />
    </Card>
  );
}
