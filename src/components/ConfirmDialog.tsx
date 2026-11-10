import { useEffect, type ReactNode } from "react";
import Button from "./Button";

export interface ConfirmRequest {
  title: string;
  body: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
}

/** Hộp xác nhận modal. Esc = huỷ. Nút xác nhận không tự focus để tránh Enter nhầm. */
export default function ConfirmDialog({ req, onClose }: { req: ConfirmRequest | null; onClose: () => void }) {
  useEffect(() => {
    if (!req) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [req, onClose]);

  if (!req) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        className="w-full max-w-md rounded-lg bg-white p-5 shadow-xl dark:bg-[#1c1f26]"
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
