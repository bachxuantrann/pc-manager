import { useState } from "react";
import Button from "./Button";
import { Notice } from "./Card";
import ProgressBar from "./ProgressBar";
import { useNavigate } from "../hooks/useNavigate";
import { useUpdate } from "../hooks/useUpdate";
import { formatBytes, formatDate } from "../lib/format";

/** Thẻ "Cập nhật" trong Cài đặt: công tắc tự kiểm tra, nút kiểm tra, cài bản mới. */
export function UpdateSettings() {
  const { info, error, checking, installing, progress, check, setAuto, install } = useUpdate();
  const a = info?.available ?? null;
  const pct = progress?.total ? (progress.downloaded / progress.total) * 100 : null;
  return (
    <div className="grid gap-3 text-sm">
      <div className="flex flex-wrap items-center gap-3">
        <span>
          Phiên bản hiện tại: <b>{info?.current ?? "…"}</b>
        </span>
        <Button variant="ghost" size="sm" loading={checking} disabled={installing} onClick={() => check(true)}>
          Kiểm tra cập nhật
        </Button>
        {info?.lastCheckMs != null && !checking && (
          <span className="text-xs opacity-60">Lần kiểm tra gần nhất: {new Date(info.lastCheckMs).toLocaleString("vi-VN")}</span>
        )}
      </div>
      <label className="flex cursor-pointer items-center gap-2">
        <input
          type="checkbox"
          className="accent-[var(--color-accent)]"
          checked={info?.auto ?? true}
          disabled={!info}
          onChange={(e) => setAuto(e.target.checked)}
        />
        Tự kiểm tra bản mới khi mở PC Manager (tối đa 1 lần/ngày)
      </label>
      <p className="text-xs opacity-60">
        Chỉ gửi một yêu cầu tới github.com để đọc phiên bản mới nhất — không gửi dữ liệu gì về máy bạn. Không bao giờ tự cài: bản mới
        chỉ được tải khi bạn bấm Cập nhật, chữ ký được kiểm tra trước khi chạy bộ cài, và Windows sẽ hỏi UAC.
      </p>
      {a && (
        <div className="grid gap-2 rounded-xl border border-accent/40 p-3">
          <div className="font-medium">
            Có bản mới {a.version}
            {a.dateMs != null && <span className="font-normal opacity-60"> · phát hành {formatDate(a.dateMs)}</span>}
          </div>
          {a.notes && (
            // "Có gì mới" là markdown của CHANGELOG: chỉ làm đậm dòng tiêu đề, phần còn lại hiện như chữ thường.
            <div className="max-h-48 overflow-auto text-xs opacity-80">
              {a.notes.split("\n").map((line, i) => {
                const h = line.match(/^#{1,6}\s+(.*)$/);
                return h ? (
                  <div key={i} className="mt-1 font-semibold first:mt-0">
                    {h[1]}
                  </div>
                ) : (
                  <div key={i} className="whitespace-pre-wrap">
                    {line.replace(/\*\*(.+?)\*\*/g, "$1") || " "}
                  </div>
                );
              })}
            </div>
          )}
          {installing ? (
            <ProgressBar
              pct={pct}
              label={
                progress
                  ? `Đang tải ${formatBytes(progress.downloaded)}${progress.total ? ` / ${formatBytes(progress.total)}` : ""} — xong sẽ chạy bộ cài (UAC)`
                  : "Đang bắt đầu tải…"
              }
            />
          ) : (
            <div>
              <Button size="sm" disabled={!info?.configured} onClick={install}>
                Cập nhật lên {a.version}
              </Button>
            </div>
          )}
          {!info?.configured && (
            <Notice tone="warn">Bản PC Manager này chưa có khoá kiểm chữ ký cập nhật — hãy tải bản mới từ trang GitHub Release.</Notice>
          )}
        </div>
      )}
      {info && !a && info.lastCheckMs != null && !checking && !error && <p className="text-xs opacity-60">Bạn đang dùng bản mới nhất.</p>}
      {error && <Notice tone="error">{error}</Notice>}
    </div>
  );
}

/** Dải báo bản mới ở đầu cửa sổ (ẩn được trong phiên này). */
export function UpdateBanner() {
  const { info, installing } = useUpdate();
  const navigate = useNavigate();
  const [hidden, setHidden] = useState<string | null>(null);
  const a = info?.available;
  if (!a || hidden === a.version || installing) return null;
  return (
    <div className="mb-4">
      <Notice tone="ok" onClose={() => setHidden(a.version)}>
        <span className="flex flex-wrap items-center gap-2">
          Có bản PC Manager mới: <b>{a.version}</b> (đang dùng {info?.current}).
          <Button size="sm" variant="ghost" onClick={() => navigate("settings")}>
            Xem & cập nhật
          </Button>
        </span>
      </Notice>
    </div>
  );
}
