import { useState } from "react";
import { api, errorMessage, type BoostMode, type BoostReport } from "../lib/api";
import { formatBytes } from "../lib/format";
import { useMemoryFeed } from "../hooks/useMemoryFeed";
import { Notice } from "./Card";

const TRIGGER_LABEL: Record<BoostReport["trigger"], string> = {
  manual: "Boost",
  smart: "Smart Boost",
  tray: "Boost từ khay hệ thống",
  auto: "Smart Boost tự động",
};

const MODE_LABEL: Record<BoostMode, string> = { light: "nhẹ", deep: "sâu" };

/**
 * Báo cáo trung thực: số chính là RAM *khả dụng* tăng thêm (lợi ích thật).
 * Cache standby đã xoá được ghi riêng — Windows vốn đã nhường phần đó cho app.
 */
export function BoostResult({ report }: { report: BoostReport }) {
  const g = report.gain;
  return (
    <Notice tone={report.warnings.length ? "warn" : "ok"}>
      <div>
        <b>
          {TRIGGER_LABEL[report.trigger]} ({MODE_LABEL[report.mode]})
        </b>
        : RAM khả dụng tăng <b>{formatBytes(g.availableGain)}</b> · {report.durationMs} ms
      </div>
      <div className="mt-1 text-xs opacity-80">
        {g.modifiedFlushed != null && <>Ghi {formatBytes(g.modifiedFlushed)} modified ra đĩa · </>}
        {g.standbyCleared != null && <>Xoá {formatBytes(g.standbyCleared)} cache standby · </>}
        {g.freeGain != null && <>Free tăng {formatBytes(g.freeGain)}</>}
      </div>
      {report.warnings.length > 0 && <div className="mt-1 text-xs">{report.warnings.join(" · ")}</div>}
    </Notice>
  );
}

function DeepConfirm({ standby, onCancel, onConfirm }: { standby: number | null; onCancel: () => void; onConfirm: () => void }) {
  return (
    <div className="rounded-md border border-amber-500/40 bg-amber-50 p-3 text-sm dark:bg-amber-500/10">
      <p>
        <b>Boost sâu</b> sẽ xoá toàn bộ cache standby{standby != null && <> (khoảng {formatBytes(standby)})</>}.
        Phần này Windows đã sẵn sàng nhường cho ứng dụng nên RAM khả dụng hầu như không tăng thêm,
        đổi lại lần mở file và ứng dụng tiếp theo sẽ chậm hơn vì phải đọc lại từ ổ đĩa.
      </p>
      <p className="mt-1 text-xs opacity-70">Chỉ nên dùng khi bạn cần nhiều RAM trống ngay, ví dụ trước khi chạy game hay máy ảo.</p>
      <div className="mt-3 flex gap-2">
        <button type="button" onClick={onConfirm} className="rounded-md bg-amber-600 px-3 py-1.5 text-sm font-medium text-white">
          Vẫn Boost sâu
        </button>
        <button type="button" onClick={onCancel} className="rounded-md px-3 py-1.5 text-sm hover:bg-black/5 dark:hover:bg-white/10">
          Huỷ
        </button>
      </div>
    </div>
  );
}

/** Nút Boost nhẹ / Boost sâu / Smart Boost, hiển thị kết quả gần nhất (kể cả từ tray). */
export default function BoostPanel() {
  const { overview, lastBoost } = useMemoryFeed();
  const [busy, setBusy] = useState<BoostMode | "smart" | null>(null);
  const [confirmDeep, setConfirmDeep] = useState(false);
  const [msg, setMsg] = useState<{ tone: "warn" | "error"; text: string } | null>(null);
  const privileged = overview?.privileged ?? false;
  const disabled = !privileged || busy !== null;

  const run = async (kind: BoostMode | "smart") => {
    setBusy(kind);
    setMsg(null);
    setConfirmDeep(false);
    try {
      if (kind === "smart") {
        const r = await api.memorySmartBoost();
        if (r.kind === "skipped") {
          setMsg({
            tone: "warn",
            text: `RAM khả dụng còn ${r.availablePct.toFixed(0)}% (ngưỡng ${r.thresholdPct}%), chưa cần Boost.`,
          });
        }
      } else {
        await api.memoryBoost(kind);
      }
      // Báo cáo đến qua event memory://boosted → lastBoost.
    } catch (e) {
      setMsg({ tone: "error", text: errorMessage(e) });
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          disabled={disabled}
          onClick={() => run("light")}
          className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
          title="Ghi trang modified ra đĩa và xoá cache standby ưu tiên thấp. Giữ cache của app hay dùng."
        >
          {busy === "light" ? "Đang Boost…" : "Boost"}
        </button>
        <button
          type="button"
          disabled={disabled}
          onClick={() => run("smart")}
          className="rounded-md border border-accent px-4 py-2 text-sm font-medium text-accent disabled:opacity-50"
          title="Chỉ Boost (sâu) khi RAM khả dụng thấp hơn ngưỡng cấu hình"
        >
          {busy === "smart" ? "Đang kiểm tra…" : "Smart Boost"}
        </button>
        <button
          type="button"
          disabled={disabled}
          onClick={() => setConfirmDeep(true)}
          className="rounded-md px-4 py-2 text-sm font-medium text-amber-700 hover:bg-amber-500/10 disabled:opacity-50 dark:text-amber-300"
        >
          {busy === "deep" ? "Đang Boost sâu…" : "Boost sâu…"}
        </button>
      </div>
      {confirmDeep && (
        <DeepConfirm
          standby={overview?.status.standby ?? null}
          onCancel={() => setConfirmDeep(false)}
          onConfirm={() => run("deep")}
        />
      )}
      {overview && !privileged && (
        <Notice tone="warn">Cần chạy PC Manager với quyền Administrator để Boost và đọc standby/modified.</Notice>
      )}
      {msg && <Notice tone={msg.tone}>{msg.text}</Notice>}
      {lastBoost && <BoostResult report={lastBoost} />}
    </div>
  );
}
