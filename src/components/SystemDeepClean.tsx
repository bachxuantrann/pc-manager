import { useEffect, useState } from "react";
import Button, { Spinner } from "./Button";
import { Card, Notice } from "./Card";
import ConfirmDialog, { type ConfirmRequest } from "./ConfirmDialog";
import { AdminNotice, useElevation } from "./Elevation";
import { api, errorMessage, type SystemCleanReport, type WinSxsAnalysis, type WindowsOldInfo } from "../lib/api";
import { formatBytes } from "../lib/format";

type Busy = null | "analyze" | "winsxs" | "winold";

function freedText(r: SystemCleanReport): string {
  const freed = r.freedBytes > 0 ? `giải phóng khoảng ${formatBytes(r.freedBytes)}` : "không thấy dung lượng trống tăng thêm";
  return `Xong sau ${Math.round(r.durationMs / 1000)} giây — ${freed}.${r.rebootRequired ? " Cần khởi động lại máy để hoàn tất." : ""}`;
}

/**
 * Dọn sâu hệ thống bằng công cụ chính thức của Windows:
 * WinSxS qua DISM, Windows.old qua Disk Cleanup. Cần quyền Administrator.
 */
export default function SystemDeepClean() {
  const { elevated } = useElevation();
  const [busy, setBusy] = useState<Busy>(null);
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
    setMsg(null);
    try {
      setAnalysis(await api.systemAnalyzeWinsxs());
    } catch (e) {
      setMsg({ tone: "error", text: errorMessage(e) });
    } finally {
      setBusy(null);
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
          <p className="text-xs opacity-70">Không dùng /ResetBase: các bản cập nhật đã cài vẫn gỡ được như bình thường.</p>
        </div>
      ),
      confirmLabel: "Bắt đầu dọn",
      onConfirm: async () => {
        setBusy("winsxs");
        setProgress(0);
        setMsg(null);
        try {
          const r = await api.systemCleanWinsxs(setProgress);
          setMsg({ tone: "ok", text: `WinSxS: ${freedText(r)}` });
          setAnalysis(null);
        } catch (e) {
          setMsg({ tone: "error", text: `WinSxS: ${errorMessage(e)}` });
        } finally {
          setBusy(null);
          setProgress(null);
        }
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
        setMsg(null);
        try {
          const r = await api.systemCleanWindowsOld();
          setMsg({ tone: "ok", text: `Windows.old: ${freedText(r)}` });
          setWinOld(await api.systemWindowsOldInfo());
        } catch (e) {
          setMsg({ tone: "error", text: `Windows.old: ${errorMessage(e)}` });
        } finally {
          setBusy(null);
        }
      },
    });

  return (
    <Card title="Dọn sâu hệ thống" className="mt-4">
      <div className="grid gap-3">
        <AdminNotice>Dọn sâu hệ thống cần quyền Administrator.</AdminNotice>

        <div className="flex flex-wrap items-start justify-between gap-3 rounded-md border border-black/5 p-3 dark:border-white/10">
          <div className="min-w-0 flex-1 text-sm">
            <div className="font-medium">Kho thành phần Windows (WinSxS)</div>
            <div className="text-xs opacity-70">
              Dọn bằng DISM — cách duy nhất an toàn. Không bao giờ xoá trực tiếp file trong WinSxS.
            </div>
            {analysis && (
              <div className="mt-1 text-xs">
                {analysis.actualSize != null ? (
                  <>
                    Dung lượng thực: <b>{analysis.actualSize}</b> ·{" "}
                    {analysis.recommended ? <b className="text-amber-700 dark:text-amber-300">DISM khuyên nên dọn</b> : "chưa cần dọn"}
                  </>
                ) : (
                  <pre className="whitespace-pre-wrap opacity-70">{analysis.raw}</pre>
                )}
              </div>
            )}
            {busy === "winsxs" && (
              <div className="mt-2">
                <div className="h-2 overflow-hidden rounded-full bg-black/10 dark:bg-white/10">
                  <div className="h-full bg-accent transition-all" style={{ width: `${progress ?? 0}%` }} />
                </div>
                <div className="mt-1 text-xs opacity-70">DISM đang chạy… {progress != null ? `${progress.toFixed(1)}%` : ""} — đừng tắt máy.</div>
              </div>
            )}
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

        <div className="flex flex-wrap items-start justify-between gap-3 rounded-md border border-black/5 p-3 dark:border-white/10">
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
          </div>
          <Button variant="danger" size="sm" disabled={locked || !winOld?.exists} loading={busy === "winold"} onClick={cleanWinOld}>
            Xoá Windows.old
          </Button>
        </div>

        {msg && <Notice tone={msg.tone}>{msg.text}</Notice>}
      </div>
      <ConfirmDialog req={confirm} onClose={() => setConfirm(null)} />
    </Card>
  );
}
