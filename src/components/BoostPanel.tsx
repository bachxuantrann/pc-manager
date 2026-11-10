import { useEffect, useState, type ReactNode } from "react";
import { api, errorMessage, type BoostReport } from "../lib/api";
import { formatBytes, pct } from "../lib/format";
import { useMemoryFeed } from "../hooks/useMemoryFeed";
import Button from "./Button";
import { Notice } from "./Card";

export const MEMORY_SETTINGS_CHANGED = "pcm:memory-settings-changed";

type CardId = "light" | "smart" | "deep";

type CardResult =
  | { kind: "report"; report: BoostReport; at: number }
  | { kind: "info"; text: string }
  | { kind: "error"; text: string };

const SOURCE_LABEL: Record<BoostReport["trigger"], string> = {
  manual: "",
  smart: "",
  tray: "từ khay hệ thống",
  auto: "tự động",
};

/** Báo cáo của mỗi lần Boost thuộc về thẻ nào (kể cả Boost từ tray / tự động). */
function cardOf(r: BoostReport): CardId {
  if (r.trigger === "smart" || r.trigger === "auto") return "smart";
  return r.mode === "deep" ? "deep" : "light";
}

function ReportView({ report }: { report: BoostReport }) {
  const g = report.gain;
  const source = SOURCE_LABEL[report.trigger];
  return (
    <div className="rounded-md bg-green-500/10 px-3 py-2 text-sm">
      <div className="flex items-baseline gap-2">
        <span className="text-lg font-semibold text-green-700 tabular-nums dark:text-green-300">+{formatBytes(g.availableGain)}</span>
        <span className="text-xs opacity-70">RAM khả dụng{source && ` · ${source}`}</span>
      </div>
      <ul className="mt-1 space-y-0.5 text-xs opacity-75">
        {g.modifiedFlushed != null && <li>Ghi {formatBytes(g.modifiedFlushed)} dữ liệu chờ ra đĩa</li>}
        {g.standbyCleared != null && <li>Xoá {formatBytes(g.standbyCleared)} cache standby</li>}
        <li>Xong trong {report.durationMs} ms</li>
      </ul>
      {report.warnings.length > 0 && <div className="mt-1 text-xs text-amber-700 dark:text-amber-300">{report.warnings.join(" · ")}</div>}
    </div>
  );
}

function ResultView({ r }: { r: CardResult | undefined }) {
  if (!r) return null;
  if (r.kind === "report") return <ReportView report={r.report} />;
  return (
    <div
      className={`rounded-md px-3 py-2 text-sm ${
        r.kind === "error" ? "bg-red-500/10 text-red-700 dark:text-red-300" : "bg-black/5 dark:bg-white/5"
      }`}
    >
      {r.text}
    </div>
  );
}

function BoostCard({
  title,
  badge,
  description,
  children,
  result,
}: {
  title: string;
  badge?: ReactNode;
  description: ReactNode;
  children: ReactNode;
  result?: CardResult;
}) {
  return (
    // Mô tả có chiều cao tối thiểu cố định để nút của 3 thẻ luôn thẳng hàng và
    // KHÔNG bị đẩy xuống khi thẻ bên cạnh hiện kết quả.
    <div className="flex flex-col gap-3 rounded-lg border border-black/10 p-4 dark:border-white/10">
      <div className="lg:min-h-[6.5rem]">
        <div className="flex items-center gap-2">
          <h3 className="font-semibold">{title}</h3>
          {badge}
        </div>
        <p className="mt-1 text-xs leading-relaxed opacity-70">{description}</p>
      </div>
      <div className="flex flex-wrap items-center gap-2">{children}</div>
      <ResultView r={result} />
    </div>
  );
}

/** Ba thẻ Boost độc lập: mỗi thẻ có nút, trạng thái chạy và kết quả của riêng nó. */
export default function BoostPanel() {
  const { overview, lastBoost } = useMemoryFeed();
  const [busy, setBusy] = useState<CardId | null>(null);
  const [confirmDeep, setConfirmDeep] = useState(false);
  const [results, setResults] = useState<Partial<Record<CardId, CardResult>>>({});
  const [threshold, setThreshold] = useState<number | null>(null);

  const privileged = overview?.privileged ?? false;
  const s = overview?.status;
  const availablePct = s ? pct(s.available, s.total) : null;
  const needsBoost = availablePct != null && threshold != null && availablePct < threshold;

  useEffect(() => {
    const load = () => api.memorySettings().then((x) => setThreshold(x.smartThresholdPct)).catch(() => setThreshold(15));
    load();
    // Trang Bộ nhớ phát sự kiện này sau khi lưu ngưỡng mới.
    window.addEventListener(MEMORY_SETTINGS_CHANGED, load);
    return () => window.removeEventListener(MEMORY_SETTINGS_CHANGED, load);
  }, []);

  // Mọi báo cáo (từ nút, tray hay auto) đến qua event → gắn vào đúng thẻ.
  useEffect(() => {
    if (lastBoost) setResults((r) => ({ ...r, [cardOf(lastBoost)]: { kind: "report", report: lastBoost, at: Date.now() } }));
  }, [lastBoost]);

  const setResult = (id: CardId, r: CardResult) => setResults((x) => ({ ...x, [id]: r }));

  const run = async (id: CardId) => {
    setBusy(id);
    setConfirmDeep(false);
    setResults((x) => ({ ...x, [id]: undefined }));
    try {
      if (id === "smart") {
        const r = await api.memorySmartBoost();
        if (r.kind === "skipped") {
          setResult("smart", {
            kind: "info",
            text: `Chưa cần Boost: RAM khả dụng còn ${r.availablePct.toFixed(0)}%, cao hơn ngưỡng ${r.thresholdPct}%.`,
          });
        }
      } else {
        await api.memoryBoost(id);
      }
    } catch (e) {
      setResult(id, { kind: "error", text: errorMessage(e) });
    } finally {
      setBusy(null);
    }
  };

  const locked = !privileged || busy !== null;

  return (
    <div className="flex flex-col gap-3">
      {overview && !privileged && (
        <Notice tone="warn">Cần chạy PC Manager với quyền Administrator để Boost và đọc standby/modified.</Notice>
      )}
      <div className="grid items-start gap-3 lg:grid-cols-3">
        <BoostCard
          title="Boost"
          badge={<span className="rounded bg-accent/15 px-1.5 py-0.5 text-[11px] font-medium text-accent">Khuyên dùng</span>}
          description="Ghi dữ liệu đang chờ ra đĩa và xoá cache ít dùng. Giữ cache của ứng dụng hay dùng nên không làm chậm máy."
          result={results.light}
        >
          <Button variant="primary" loading={busy === "light"} disabled={locked} onClick={() => run("light")}>
            {busy === "light" ? "Đang Boost…" : "Boost ngay"}
          </Button>
        </BoostCard>

        <BoostCard
          title="Smart Boost"
          badge={
            availablePct != null &&
            threshold != null && (
              <span
                className={`rounded px-1.5 py-0.5 text-[11px] font-medium ${
                  needsBoost ? "bg-amber-500/15 text-amber-700 dark:text-amber-300" : "bg-green-500/15 text-green-700 dark:text-green-300"
                }`}
              >
                {needsBoost ? "Nên Boost" : "RAM còn đủ"}
              </span>
            )
          }
          description={
            <>
              Chỉ chạy khi RAM khả dụng dưới ngưỡng{threshold != null && <> <b>{threshold}%</b></>}
              {availablePct != null && <> · hiện còn <b>{availablePct.toFixed(0)}%</b></>}. Khi chạy sẽ xoá toàn bộ cache để ứng dụng có RAM ngay.
            </>
          }
          result={results.smart}
        >
          <Button variant="secondary" loading={busy === "smart"} disabled={locked} onClick={() => run("smart")}>
            {busy === "smart" ? "Đang kiểm tra…" : "Kiểm tra & Boost"}
          </Button>
        </BoostCard>

        <BoostCard
          title="Boost sâu"
          description={
            <>
              Xoá toàn bộ cache standby{s?.standby != null && <> (khoảng <b>{formatBytes(s.standby)}</b>)</>}. RAM trống tăng mạnh nhưng
              RAM khả dụng hầu như không đổi, lần mở ứng dụng sau sẽ chậm hơn.
            </>
          }
          result={results.deep}
        >
          {confirmDeep ? (
            <>
              <Button variant="warning" loading={busy === "deep"} disabled={locked} onClick={() => run("deep")}>
                Xác nhận Boost sâu
              </Button>
              <Button variant="ghost" disabled={busy !== null} onClick={() => setConfirmDeep(false)}>
                Huỷ
              </Button>
            </>
          ) : (
            <Button
              variant="secondary"
              loading={busy === "deep"}
              disabled={locked}
              onClick={() => {
                setResults((x) => ({ ...x, deep: undefined }));
                setConfirmDeep(true);
              }}
            >
              {busy === "deep" ? "Đang Boost sâu…" : "Boost sâu…"}
            </Button>
          )}
        </BoostCard>
      </div>
    </div>
  );
}
