import { DIALOG } from "./ui";
import { useEffect, type ReactNode } from "react";
import Button from "./Button";
import { usePageActive } from "../hooks/usePageActive";

export interface ConfirmRequest {
  title: string;
  body: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
}

/** Hộp xác nhận modal. Esc = huỷ. Nút xác nhận không tự focus để tránh Enter nhầm. */
export default function ConfirmDialog({ req, onClose }: { req: ConfirmRequest | null; onClose: () => void }) {
  // Trang đang ẩn (vẫn giữ sống) => không nhận phím của trang khác.
  const active = usePageActive();
  useEffect(() => {
    if (!req || !active) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [req, onClose, active]);

  if (!req) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        className={`w-full max-w-md p-5 ${DIALOG}`}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-base font-semibold">{req.title}</h2>
        <div className="mt-2 text-sm opacity-80">{req.body}</div>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Huỷ
          </Button>
          <Button
            variant={req.danger ? "danger" : "primary"}
            onClick={() => {
              onClose();
              req.onConfirm();
            }}
          >
            {req.confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
