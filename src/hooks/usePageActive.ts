import { createContext, useContext, useEffect, useId } from "react";

/**
 * Trang đang được chọn hay đang ẩn (App giữ sống các trang đã mở để không mất trạng thái tác vụ
 * đang chạy khi chuyển tab). Ngoài một trang (provider toàn cục…) luôn là `true`.
 */
export const PageActiveContext = createContext(true);

export function usePageActive(): boolean {
  return useContext(PageActiveContext);
}

/** Báo cho App biết một tác vụ trong trang đang chạy (`key` phân biệt nhiều tác vụ trong một trang). */
export type ReportBusy = (key: string, busy: boolean) => void;

export const PageBusyContext = createContext<ReportBusy>(() => {});

/**
 * Đánh dấu trang chứa component là "đang chạy" khi `busy` = true — Sidebar hiện dấu xoay cạnh
 * tên trang để người dùng biết tab nào còn tác vụ khi đã chuyển sang tab khác.
 */
export function useReportBusy(busy: boolean) {
  const report = useContext(PageBusyContext);
  const key = useId();
  useEffect(() => {
    report(key, busy);
    return () => report(key, false);
  }, [report, key, busy]);
}
