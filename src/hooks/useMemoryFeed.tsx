import { createContext, useCallback, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
import { api, errorMessage, type BoostReport, type MemoryOverview } from "../lib/api";
import { usePageActive } from "./usePageActive";

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

/**
 * Kho dữ liệu RAM dùng chung. Trang tiêu thụ chỉ đăng ký (nhận cập nhật + tính là "đang xem") khi
 * đang được chọn => trang ẩn nhưng giữ sống không vẽ lại, và không trang nào xem thì không hỏi backend.
 */
class FeedStore {
  private feed: Feed;
  private listeners = new Set<() => void>();
  private consumers = 0;
  private consumerListeners = new Set<(n: number) => void>();
  private inflight = false;

  constructor() {
    this.feed = { overview: null, history: [], error: null, lastBoost: null, refresh: this.refresh };
  }

  get = () => this.feed;

  subscribe = (cb: () => void) => {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  };

  private set(patch: Partial<Feed>) {
    this.feed = { ...this.feed, ...patch };
    this.listeners.forEach((l) => l());
  }

  addConsumer = () => {
    this.consumers += 1;
    this.consumerListeners.forEach((l) => l(this.consumers));
    return () => {
      this.consumers -= 1;
      this.consumerListeners.forEach((l) => l(this.consumers));
    };
  };

  onConsumers = (cb: (n: number) => void) => {
    this.consumerListeners.add(cb);
    cb(this.consumers);
    return () => {
      this.consumerListeners.delete(cb);
    };
  };

  setBoost(r: BoostReport) {
    this.set({ lastBoost: r });
  }

  refresh = () => {
    if (this.inflight) return;
    this.inflight = true;
    api
      .memoryStatus()
      .then((o) => {
        const usedPct = o.status.total > 0 ? (o.status.used * 100) / o.status.total : 0;
        const now = Date.now();
        const prev = this.feed.history;
        // Ngừng lấy mẫu (rời trang / ẩn cửa sổ) => bắt đầu lịch sử mới thay vì nối qua khoảng trống.
        const fresh = prev.length > 0 && now - prev[prev.length - 1].t > SAMPLE_MS * 3;
        const next = [...(fresh ? [] : prev), { t: now, usedPct }];
        this.set({
          overview: o,
          error: null,
          history: next.length > HISTORY_LEN ? next.slice(next.length - HISTORY_LEN) : next,
        });
      })
      .catch((e) => this.set({ error: errorMessage(e) }))
      .finally(() => {
        this.inflight = false;
      });
  };
}

const Ctx = createContext<FeedStore | null>(null);

function useDocumentVisible(): boolean {
  const [visible, setVisible] = useState(() => !document.hidden);
  useEffect(() => {
    const onChange = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);
  return visible;
}

/**
 * Một nguồn dữ liệu RAM dùng chung cho Dashboard + Memory, đặt ở App. Chỉ lấy mẫu khi cửa sổ hiển
 * thị VÀ có ít nhất một trang tiêu thụ đang được chọn (ở Cài đặt, Ổ đĩa… không hỏi backend).
 */
export function MemoryFeedProvider({ children }: { children: ReactNode }) {
  const [store] = useState(() => new FeedStore());
  const [consumers, setConsumers] = useState(0);
  const visible = useDocumentVisible();
  useEffect(() => store.onConsumers(setConsumers), [store]);

  const polling = visible && consumers > 0;
  useEffect(() => {
    if (!polling) return;
    store.refresh();
    const id = window.setInterval(store.refresh, SAMPLE_MS);
    return () => window.clearInterval(id);
  }, [polling, store]);

  useEffect(() => {
    const p = api.onBoosted((r) => {
      store.setBoost(r);
      store.refresh();
    });
    return () => {
      p.then((un) => un());
    };
  }, [store]);

  return <Ctx.Provider value={store}>{children}</Ctx.Provider>;
}

export function useMemoryFeed(): Feed {
  const store = useContext(Ctx);
  if (!store) throw new Error("useMemoryFeed phải nằm trong MemoryFeedProvider");
  const active = usePageActive();
  useEffect(() => (active ? store.addConsumer() : undefined), [store, active]);
  // Trang ẩn: không đăng ký => không vẽ lại; khi được chọn lại, đọc ngay giá trị mới nhất.
  const subscribe = useCallback((cb: () => void) => (active ? store.subscribe(cb) : () => {}), [store, active]);
  return useSyncExternalStore(subscribe, store.get);
}
