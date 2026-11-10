import { useEffect, type ReactNode } from "react";

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
          <button type="button" onClick={onClose} className="rounded-md px-4 py-2 text-sm hover:bg-black/5 dark:hover:bg-white/10">
            Huỷ
          </button>
          <button
            type="button"
            onClick={() => {
              onClose();
              req.onConfirm();
            }}
            className={`rounded-md px-4 py-2 text-sm font-medium text-white ${req.danger ? "bg-red-600" : "bg-accent"}`}
          >
            {req.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
