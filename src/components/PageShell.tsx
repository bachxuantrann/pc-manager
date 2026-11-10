import type { ReactNode } from "react";

interface Props {
  title: string;
  subtitle?: string;
  children?: ReactNode;
  /** Thanh công cụ bên phải tiêu đề (tìm kiếm, bộ lọc…). */
  actions?: ReactNode;
}

/**
 * Khung trang chung: trải hết bề rộng cửa sổ và lấp đầy chiều cao để nội dung
 * giãn theo khi phóng to (không còn bị bó ở giữa màn hình).
 */
export default function PageShell({ title, subtitle, children, actions }: Props) {
  return (
    <section className="flex h-full w-full flex-col">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold 2xl:text-3xl">{title}</h1>
          {subtitle && <p className="mt-1 text-sm opacity-60">{subtitle}</p>}
        </div>
        {actions}
      </div>
      <div className="mt-4 flex min-h-0 flex-1 flex-col 2xl:mt-6">{children}</div>
    </section>
  );
}
