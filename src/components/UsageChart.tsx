import { HISTORY_LEN, type Sample } from "../hooks/useMemoryFeed";

interface Props {
  samples: Sample[];
  height?: number;
}

/** Biểu đồ vùng % RAM dùng trong 60s gần nhất. SVG thuần, không thêm thư viện. */
export default function UsageChart({ samples, height = 140 }: Props) {
  const W = 600;
  const H = height;
  const step = W / (HISTORY_LEN - 1);
  // Căn phải: mẫu mới nhất luôn ở mép phải.
  const offset = HISTORY_LEN - samples.length;
  const pts = samples.map((s, i) => [(i + offset) * step, H - (Math.min(100, s.usedPct) / 100) * H] as const);
  const line = pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const area = pts.length > 1 ? `${line} L${W},${H} L${pts[0][0].toFixed(1)},${H} Z` : "";
  const last = samples[samples.length - 1];

  return (
    <div className="relative">
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" className="block w-full" style={{ height: H }}>
        {[25, 50, 75].map((g) => (
          <line
            key={g}
            x1={0}
            x2={W}
            y1={H - (g / 100) * H}
            y2={H - (g / 100) * H}
            className="stroke-black/10 dark:stroke-white/10"
            strokeDasharray="4 4"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {area && <path d={area} className="fill-accent/15" />}
        {pts.length > 1 && (
          <path d={line} fill="none" className="stroke-accent" strokeWidth={2} vectorEffect="non-scaling-stroke" />
        )}
      </svg>
      <div className="mt-1 flex justify-between text-xs opacity-50">
        <span>60 giây trước</span>
        <span>{last ? `${last.usedPct.toFixed(0)}% đang dùng` : "đang đo…"}</span>
      </div>
    </div>
  );
}
