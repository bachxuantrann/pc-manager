import { createContext, useContext } from "react";
import type { PageId } from "../components/Sidebar";

/** Chuyển trang từ bên trong một trang (vd Smart Diagnosis mở trang Dọn rác). App cung cấp. */
export const NavigateContext = createContext<(page: PageId) => void>(() => {});

export function useNavigate(): (page: PageId) => void {
  return useContext(NavigateContext);
}
