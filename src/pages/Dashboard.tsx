import PageShell from "../components/PageShell";
import ArcGauge from "../components/ArcGauge";
import BoostPanel from "../components/BoostPanel";
import UsageChart from "../components/UsageChart";
import { Card, Notice, Stat } from "../components/Card";
import { useMemoryFeed } from "../hooks/useMemoryFeed";
import { formatBytes, pct } from "../lib/format";


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
            <ArcGauge value={s ? pct(s.used, s.total) : null} label="RAM đang dùng" className="w-48 xl:w-60 2xl:w-72" />
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
