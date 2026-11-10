// Mọi lời gọi invoke() tới backend Rust được gom tại đây, có type.
import { invoke } from "@tauri-apps/api/core";

/** Lỗi backend trả về (khớp với `AppError` bên Rust). */
export interface ApiError {
  code: string;
  message: string;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function errorMessage(e: unknown): string {
  if (isApiError(e)) return `${e.message} (${e.code})`;
  return String(e);
}

export interface AppInfo {
  version: string;
  isElevated: boolean;
  pid: number;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
};
