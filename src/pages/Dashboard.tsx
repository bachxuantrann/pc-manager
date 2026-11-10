import { useEffect, useState } from "react";
import PageShell from "../components/PageShell";
import { api, errorMessage, type AppInfo } from "../lib/api";

export default function Dashboard() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.appInfo().then(setInfo).catch((e) => setErr(errorMessage(e)));
  }, []);

  return (
    <PageShell title="Tổng quan" subtitle="Tình trạng máy — số liệu RAM sẽ có ở giai đoạn 1">
      <div className="rounded-lg bg-white p-4 shadow-sm dark:bg-white/5">
        {err && <p className="text-red-500">{err}</p>}
        {info && (
          <dl className="grid grid-cols-[10rem_1fr] gap-y-1 text-sm">
            <dt className="opacity-60">Phiên bản</dt>
            <dd>{info.version}</dd>
            <dt className="opacity-60">Quyền admin</dt>
            <dd className={info.isElevated ? "text-green-600" : "text-red-500"}>
              {info.isElevated ? "Có" : "Không — hãy chạy với quyền Administrator"}
            </dd>
            <dt className="opacity-60">PID</dt>
            <dd>{info.pid}</dd>
          </dl>
        )}
      </div>
    </PageShell>
  );
}
