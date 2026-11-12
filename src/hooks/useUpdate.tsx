import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, errorMessage, type UpdateInfo, type UpdateProgress } from "../lib/api";

interface UpdateState {
  info: UpdateInfo | null;
  error: string | null;
  checking: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  /** force = true: bấm tay "Kiểm tra cập nhật". */
  check: (force: boolean) => Promise<void>;
  setAuto: (enabled: boolean) => Promise<void>;
  install: () => Promise<void>;
}

const Ctx = createContext<UpdateState | null>(null);

/**
 * Trạng thái cập nhật dùng chung (banner ở App + thẻ trong Cài đặt). Mở cửa sổ => hỏi backend một lần;
 * backend tự quyết có gọi GitHub không (đang bật tự kiểm tra và đã quá 1 ngày). Không polling.
 */
export function UpdateProvider({ children }: { children: ReactNode }) {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const started = useRef(false);

  const check = useCallback(async (force: boolean) => {
    setChecking(true);
    setError(null);
    try {
      setInfo(await api.updateCheck(force));
    } catch (e) {
      // Tự kiểm tra lúc mở cửa sổ mà không có mạng: im lặng, chỉ báo khi người dùng bấm tay.
      if (force) setError(errorMessage(e));
    } finally {
      setChecking(false);
    }
  }, []);

  const setAuto = useCallback(async (enabled: boolean) => {
    setError(null);
    try {
      setInfo(await api.updateSetAuto(enabled));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const install = useCallback(async () => {
    setInstalling(true);
    setError(null);
    setProgress(null);
    try {
      // Thành công thì app thoát để bộ cài chạy — dòng dưới chỉ tới khi có lỗi.
      await api.updateInstall(setProgress);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setInstalling(false);
    }
  }, []);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void check(false);
  }, [check]);

  return (
    <Ctx.Provider value={{ info, error, checking, installing, progress, check, setAuto, install }}>{children}</Ctx.Provider>
  );
}

export function useUpdate(): UpdateState {
  const v = useContext(Ctx);
  if (!v) throw new Error("useUpdate phải nằm trong UpdateProvider");
  return v;
}
