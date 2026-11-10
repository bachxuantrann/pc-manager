import { HISTORY_LEN, type Sample } from "../hooks/useMemoryFeed";

interface Props {
  samples: Sample[];
  /** Chiều cao tối thiểu (px). Biểu đồ giãn theo khung cha nếu khung cao hơn. */
  minHeight?: number;
}

/**
 * Biểu đồ vùng % RAM dùng trong 60s gần nhất. SVG thuần, co giãn theo cả hai
 * chiều của khung chứa (preserveAspectRatio="none" + nét vẽ không bị kéo dãn).
 */
export default function UsageChart({ samples, minHeight = 140 }: Props) {
  // Hệ toạ độ ảo 0..100 x 0..100; SVG tự kéo theo kích thước thật.
  const W = 100;
  const H = 100;
  const step = W / (HISTORY_LEN - 1);
  const offset = HISTORY_LEN - samples.length;
  const pts = samples.map((s, i) => [(i + offset) * step, H - Math.min(100, Math.max(0, s.usedPct))] as const);
  const line = pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(2)},${y.toFixed(2)}`).join(" ");
  const area = pts.length > 1 ? `${line} L${W},${H} L${pts[0][0].toFixed(2)},${H} Z` : "";
  const last = samples[samples.length - 1];

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <div className="relative min-h-0 flex-1" style={{ minHeight }}>
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" className="absolute inset-0 h-full w-full">
          {[25, 50, 75].map((g) => (
            <line
              key={g}
              x1={0}
              x2={W}
              y1={H - g}
              y2={H - g}
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
        {/* Nhãn trục Y — HTML để chữ không bị kéo dãn theo SVG. */}
        {[75, 50, 25].map((g) => (
          <span key={g} className="absolute right-1 -translate-y-1/2 text-[10px] opacity-40" style={{ top: `${100 - g}%` }}>
            {g}%
          </span>
        ))}
      </div>
      <div className="mt-1 flex justify-between text-xs opacity-50">
        <span>60 giây trước</span>
        <span>{last ? `${last.usedPct.toFixed(0)}% đang dùng` : "đang đo…"}</span>
      </div>
    </div>
  );
}
