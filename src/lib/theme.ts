import { createContext, useContext, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Tuỳ chọn giao diện: tối (mặc định), sáng, hoặc theo cài đặt của Windows. */
export type ThemePref = "dark" | "light" | "system";

const KEY = "pcm.theme";

export function loadThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "system" || v === "dark" ? v : "dark";
  } catch {
    return "dark";
  }
}

const systemDark = () => window.matchMedia("(prefers-color-scheme: dark)").matches;

/** Đặt data-theme trên <html> + màu thanh tiêu đề cửa sổ của Windows cho khớp. */
export function applyTheme(pref: ThemePref) {
  const dark = pref === "dark" || (pref === "system" && systemDark());
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  // Ngoài Tauri (chế độ demo trên trình duyệt) không có cửa sổ để đổi — bỏ qua.
  try {
    // Gọi thẳng lệnh của Tauri thay vì import module `window` (nặng ~18 KB) chỉ để dùng một hàm.
    invoke("plugin:window|set_theme", { label: "main", value: pref === "system" ? null : dark ? "dark" : "light" }).catch(() => {});
  } catch {
    // không chạy trong Tauri
  }
}

/** Đọc/đổi tuỳ chọn giao diện; "system" tự theo khi Windows đổi sáng/tối. */
export function useThemePref(): [ThemePref, (p: ThemePref) => void] {
  const [pref, setPref] = useState<ThemePref>(loadThemePref);
  useEffect(() => {
    applyTheme(pref);
    if (pref !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme("system");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [pref]);
  const update = (p: ThemePref) => {
    try {
      localStorage.setItem(KEY, p);
    } catch {
      // không lưu được (bị chặn) — vẫn đổi cho phiên này
    }
    setPref(p);
  };
  return [pref, update];
}

/** Một nơi giữ tuỳ chọn giao diện cho cả app (App cung cấp, trang Cài đặt đọc/đổi). */
export const ThemeContext = createContext<[ThemePref, (p: ThemePref) => void]>(["dark", () => {}]);
export const useTheme = () => useContext(ThemeContext);
