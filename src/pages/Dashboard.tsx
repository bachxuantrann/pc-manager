import PageShell from "../components/PageShell";
import BoostPanel from "../components/BoostPanel";
import UsageChart from "../components/UsageChart";
import { Card, Notice, Stat } from "../components/Card";
import { useMemoryFeed } from "../hooks/useMemoryFeed";
import { formatBytes, pct } from "../lib/format";

function Gauge({ value }: { value: number }) {
  const r = 52;
  const c = 2 * Math.PI * r;
  const v = Math.min(100, Math.max(0, value));
  const tone = v >= 85 ? "stroke-red-500" : v >= 70 ? "stroke-amber-500" : "stroke-accent";
  // viewBox cố định → vòng tròn và chữ số phóng theo kích thước CSS.
  return (
    <svg viewBox="0 0 128 128" className="h-32 w-32 shrink-0 xl:h-40 xl:w-40 2xl:h-52 2xl:w-52">
      <circle cx={64} cy={64} r={r} className="fill-none stroke-black/10 dark:stroke-white/10" strokeWidth={12} />
      <circle
        cx={64}
        cy={64}
        r={r}
        className={`fill-none ${tone} transition-[stroke-dashoffset] duration-500`}
        strokeWidth={12}
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - v / 100)}
        transform="rotate(-90 64 64)"
      />
      <text x={64} y={64} dominantBaseline="central" textAnchor="middle" className="fill-current" fontSize={24} fontWeight={600}>
        {v.toFixed(0)}%
      </text>
    </svg>
  );
}

export default function Dashboard() {
  const { overview, history, error } = useMemoryFeed();
  const s = overview?.status;

  return (
    <PageShell title="Tổng quan" subtitle="Tình trạng bộ nhớ, cập nhật mỗi 2 giây khi cửa sổ đang mở">
      {error && (
        <div className="mb-4">
          <Notice tone="error">{error}</Notice>
        </div>
      )}
      {/* Hàng trên giãn theo chiều cao cửa sổ; hàng Boost giữ chiều cao tự nhiên. */}
      <div className="grid min-h-[240px] flex-1 gap-4 md:grid-cols-[auto_1fr] 2xl:gap-6">
        <Card title="RAM" className="flex flex-col">
          <div className="flex flex-1 items-center justify-center gap-6 xl:gap-8 2xl:gap-12">
            <Gauge value={s ? pct(s.used, s.total) : 0} />
            <div className="grid gap-2 2xl:gap-4">
              <Stat label="Đang dùng" value={formatBytes(s?.used)} />
              <Stat label="Còn trống" value={formatBytes(s?.available)} />
              <Stat label="Tổng" value={formatBytes(s?.total)} />
            </div>
          </div>
        </Card>
        <Card title="60 giây gần nhất" className="flex min-w-0 flex-col">
          <UsageChart samples={history} minHeight={160} />
        </Card>
      </div>
      <Card title="Tăng tốc" className="mt-4 shrink-0 2xl:mt-6">
        <BoostPanel />
      </Card>
    </PageShell>
  );
}
