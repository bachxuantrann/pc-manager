import { useEffect, useState } from "react";
import { usePageActive } from "./usePageActive";

/**
 * true khi cửa sổ đang hiển thị (không minimize / không ẩn xuống tray) VÀ trang chứa component
 * đang được chọn (trang đã mở nhưng đang ẩn vẫn được giữ sống — xem App).
 * Dùng để dừng polling khi không ai nhìn — xem CLAUDE.md.
 * WebView2 đặt document.hidden = true khi cửa sổ bị ẩn hoặc minimize.
 */
export function useWindowVisible(): boolean {
  const [visible, setVisible] = useState(() => !document.hidden);
  const active = usePageActive();
  useEffect(() => {
    const onChange = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);
  return visible && active;
}

/** Gọi `fn` mỗi `ms` mili-giây, chỉ khi cửa sổ đang hiển thị. */
export function useVisibleInterval(fn: () => void, ms: number) {
  const visible = useWindowVisible();
  useEffect(() => {
    if (!visible) return;
    fn();
    const id = window.setInterval(fn, ms);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, ms]);
}
