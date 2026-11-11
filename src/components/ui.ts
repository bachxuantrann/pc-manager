/**
 * Class dùng chung cho mọi trang để giao diện đồng bộ (theme tối mới). Một chỗ sửa => mọi trang đổi.
 * Màu nền tối đặc `#0d1015` ≈ màu nền trang (#05070b) phủ 3.5% trắng của card — dùng cho phần cần
 * nền đặc (tiêu đề bảng dính trên đầu, ô chọn, hộp thoại) để không bị lộ nội dung phía sau.
 */

/** Khung giống Card: bảng, danh sách. */
export const SURFACE =
  "rounded-2xl border border-black/5 bg-white shadow-sm dark:border-white/[0.06] dark:bg-white/[0.035] dark:shadow-none";

/** Hàng tiêu đề bảng dính trên đầu khi cuộn (cần nền đặc). */
export const THEAD = "sticky top-0 z-10 bg-white text-xs dark:bg-[#0d1015]";

/** Ô nhập chữ (tìm kiếm, đường dẫn…). */
export const INPUT =
  "h-8 rounded-lg border border-black/10 bg-white px-3 text-sm outline-none transition-colors focus:border-accent dark:border-white/10 dark:bg-white/[0.04]";

/** Ô chọn (select). */
export const SELECT =
  "h-8 cursor-pointer rounded-lg border border-black/10 bg-white px-2 text-sm outline-none focus:border-accent dark:border-white/10 dark:bg-[#0d1015]";

/** Khung con bên trong card (một mục cấu hình, một lựa chọn…). */
export const PANEL = "rounded-xl border border-black/5 dark:border-white/[0.06]";

/** Hộp thoại. */
export const DIALOG = "rounded-2xl border border-black/5 bg-white shadow-xl dark:border-white/10 dark:bg-[#0d1015]";

/** Nút lọc dạng viên (tab con, bộ lọc). */
export function pill(active: boolean): string {
  return `whitespace-nowrap rounded-full px-3 py-1 text-sm transition-colors ${
    active ? "bg-accent text-white" : "bg-black/5 hover:bg-black/10 dark:bg-white/[0.07] dark:hover:bg-white/[0.12]"
  }`;
}
