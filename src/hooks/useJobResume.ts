import { useEffect, useRef, useState } from "react";
import { api, isStartedHere, type Job, type JobKind, type JobOutcome } from "../lib/api";

export interface ResumeHandlers<P> {
  /** Tác vụ đang chạy (do cửa sổ trước khởi chạy) — hiện lại trạng thái "đang chạy". */
  onRunning: (progress: P | null) => void;
  onProgress?: (progress: P) => void;
  /** Tác vụ xong (lúc đang theo dõi, hoặc đã xong khi cửa sổ còn đóng). */
  onDone: (outcome: JobOutcome) => void;
}

/**
 * Nối lại tác vụ dài `kind` mà backend vẫn chạy sau khi cửa sổ bị đóng (xuống tray) rồi mở lại.
 * Tác vụ do chính cửa sổ này khởi chạy thì bỏ qua — trang đã nhận tiến độ/kết quả qua invoke.
 */
export function useJobResume<P>(kind: JobKind, handlers: ResumeHandlers<P>) {
  const h = useRef(handlers);
  h.current = handlers;
  useEffect(() => {
    let alive = true;
    let following = false;
    let unlisten: (() => void) | undefined;
    const deliver = (o: JobOutcome) => {
      following = false;
      h.current.onDone(o);
      api.jobsAck(kind).catch(() => {});
    };
    const handle = (j: Job) => {
      if (!alive || j.kind !== kind || isStartedHere(kind)) return;
      if (j.running) {
        if (!following) {
          following = true;
          h.current.onRunning((j.progress as P) ?? null);
        } else if (j.progress != null) {
          h.current.onProgress?.(j.progress as P);
        }
      } else if (j.outcome && following) {
        deliver(j.outcome);
      }
    };
    api
      .onJob(handle)
      .then((u) => (alive ? (unlisten = u) : u()))
      .catch(() => {});
    api
      .jobsSnapshot()
      .then((list) => {
        const j = list.find((x) => x.kind === kind);
        if (!j || !alive || isStartedHere(kind)) return;
        if (j.running) handle(j);
        else if (j.outcome) deliver(j.outcome);
      })
      .catch(() => {});
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [kind]);
}

/** Các loại tác vụ đang chạy trong backend (cho Sidebar — kể cả trang chưa mở lại). */
export function useRunningJobs(): Set<JobKind> {
  const [running, setRunning] = useState<Set<JobKind>>(new Set());
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    const apply = (j: Job) =>
      setRunning((prev) => {
        if (prev.has(j.kind) === j.running) return prev;
        const next = new Set(prev);
        if (j.running) next.add(j.kind);
        else next.delete(j.kind);
        return next;
      });
    api
      .onJob((j) => alive && apply(j))
      .then((u) => (alive ? (unlisten = u) : u()))
      .catch(() => {});
    api
      .jobsSnapshot()
      .then((list) => alive && list.forEach(apply))
      .catch(() => {});
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);
  return running;
}
