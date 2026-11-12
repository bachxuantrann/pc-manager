import { IconCheck } from "./Icons";
import type { Publisher } from "../lib/api";
import { publisherText } from "../lib/processTable";

/** Ô "Publisher" dùng chung (Tiến trình, Khởi động): dấu kiểm khi đã ký, màu theo trạng thái chữ ký. */
export default function PublisherCell({ p, empty = "" }: { p: Publisher; empty?: string }) {
  const cls =
    p.status === "signed"
      ? ""
      : p.status === "unsigned"
        ? "text-amber-600 dark:text-amber-400"
        : p.status === "invalid"
          ? "text-red-600 dark:text-red-400"
          : "opacity-50";
  return (
    <span className={cls} title={p.status === "invalid" ? `Mã lỗi WinVerifyTrust ${p.code}` : undefined}>
      {p.status === "signed" && <IconCheck className="mr-1 inline-block h-[1em] w-[1em] align-[-0.125em] text-green-600" />}
      {publisherText(p) || empty}
    </span>
  );
}
