import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, errorMessage, type BoostReport, type MemoryOverview } from "../lib/api";
import { useVisibleInterval } from "./useWindowVisible";

/** Refresh 2s khi cửa sổ hiển thị (CLAUDE.md) → 30 mẫu = 60 giây. */
export const SAMPLE_MS = 2000;
export const HISTORY_LEN = 30;

export interface Sample {
  t: number;
  usedPct: number;
}

interface Feed {
  overview: MemoryOverview | null;
  history: Sample[];
  error: string | null;
  lastBoost: BoostReport | null;
  refresh: () => void;
}

const Ctx = createContext<Feed | null>(null);

/**
 * Một nguồn dữ liệu RAM dùng chung cho Dashboard + Memory, đặt ở App để lịch sử
 * 60s không mất khi chuyển trang. Dừng hẳn khi cửa sổ ẩn/minimize.
 */
export function MemoryFeedProvider({ children }: { children: ReactNode }) {
  const [overview, setOverview] = useState<MemoryOverview | null>(null);
  const [history, setHistory] = useState<Sample[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [lastBoost, setLastBoost] = useState<BoostReport | null>(null);
  const inflight = useRef(false);

  const refresh = useCallback(() => {
    if (inflight.current) return;
    inflight.current = true;
    api
      .memoryStatus()
      .then((o) => {
        setOverview(o);
        setError(null);
        const usedPct = o.status.total > 0 ? (o.status.used * 100) / o.status.total : 0;
        setHistory((h) => {
          const next = [...h, { t: Date.now(), usedPct }];
          return next.length > HISTORY_LEN ? next.slice(next.length - HISTORY_LEN) : next;
        });
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => {
        inflight.current = false;
      });
  }, []);

  useVisibleInterval(refresh, SAMPLE_MS);

  useEffect(() => {
    const p = api.onBoosted((r) => {
      setLastBoost(r);
      refresh();
    });
    return () => {
      p.then((un) => un());
    };
  }, [refresh]);

  return (
    <Ctx.Provider value={{ overview, history, error, lastBoost, refresh }}>{children}</Ctx.Provider>
  );
}

export function useMemoryFeed(): Feed {
  const v = useContext(Ctx);
  if (!v) throw new Error("useMemoryFeed phải nằm trong MemoryFeedProvider");
  return v;
}
