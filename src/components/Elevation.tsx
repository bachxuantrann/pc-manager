import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import Button from "./Button";
import { Notice } from "./Card";
import { api, errorMessage, isApiError } from "../lib/api";

interface Elevation {
  /** null = chưa biết (đang hỏi backend). */
  elevated: boolean | null;
  /** Trang đang mở — mở lại đúng trang này sau khi chạy với quyền admin. */
  page: string;
}

const Ctx = createContext<Elevation>({ elevated: null, page: "dashboard" });

/** Hỏi backend app có quyền admin không; báo trang cần mở sẵn (sau khi vừa nâng quyền). */
export function ElevationProvider({ page, onStartPage, children }: { page: string; onStartPage: (p: string) => void; children: ReactNode }) {
  const [elevated, setElevated] = useState<boolean | null>(null);
  useEffect(() => {
    api
      .appInfo()
      .then((i) => {
        setElevated(i.isElevated);
        if (i.startPage) onStartPage(i.startPage);
      })
      .catch(() => setElevated(null));
    // Chỉ hỏi một lần khi cửa sổ mở.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return <Ctx.Provider value={{ elevated, page }}>{children}</Ctx.Provider>;
}

export const useElevation = () => useContext(Ctx);

/**
 * Nút mở lại PC Manager với quyền Administrator (Windows hỏi UAC).
 * Không hiện gì khi app đã có quyền admin.
 */
export function ElevateButton({ size = "sm", label = "Chạy với quyền Administrator" }: { size?: "sm" | "md"; label?: string }) {
  const { elevated, page } = useElevation();
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  if (elevated !== false) return null;
  const run = async () => {
    setBusy(true);
    setErr(null);
    try {
      await api.relaunchElevated(page);
      // Thành công: app sẽ tự đóng và mở lại với quyền admin.
    } catch (e) {
      setErr(isApiError(e) && e.code === "cancelled" ? "Đã huỷ ở hộp UAC" : errorMessage(e));
      setBusy(false);
    }
  };
  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <Button variant="warning" size={size} loading={busy} onClick={run} title="Windows sẽ hỏi xác nhận (UAC), rồi PC Manager mở lại với quyền Administrator">
        🛡 {label}
      </Button>
      {err && <span className="text-xs opacity-80">{err}</span>}
    </span>
  );
}

/** Thông báo "cần quyền Administrator" kèm nút nâng quyền. Ẩn khi đã có quyền. */
export function AdminNotice({ children }: { children: ReactNode }) {
  const { elevated } = useElevation();
  if (elevated !== false) return null;
  return (
    <Notice tone="warn">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span>{children}</span>
        <ElevateButton />
      </div>
    </Notice>
  );
}
