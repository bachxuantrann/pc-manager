/**
 * Đồng hồ dạng vòng cung chia vạch (kiểu bảng điều khiển). SVG tĩnh: chỉ vẽ lại khi `value` đổi,
 * không animation lặp, không blur — WebView chạy không GPU nên mọi hiệu ứng do CPU vẽ.
 */
const TICKS = 44;
const START = 200; // độ (0° = hướng 3 giờ, ngược chiều kim đồng hồ)
const SWEEP = 220;
const CX = 100;
const CY = 100;

function point(angleDeg: number, r: number): [number, number] {
  const a = (angleDeg * Math.PI) / 180;
  return [CX + r * Math.cos(a), CY - r * Math.sin(a)];
}

export default function ArcGauge({
  value,
  unit = "%",
  label,
  warnAt = 70,
  dangerAt = 85,
  className = "",
}: {
  /** 0–100. */
  value: number | null;
  unit?: string;
  label: string;
  warnAt?: number;
  dangerAt?: number;
  className?: string;
}) {
  const v = value == null ? 0 : Math.min(100, Math.max(0, value));
  const lit = Math.round((v / 100) * TICKS);
  const tone = v >= dangerAt ? "text-red-500" : v >= warnAt ? "text-amber-500" : "text-accent";
  return (
    <svg viewBox="0 0 200 150" className={`w-full max-w-[260px] ${className}`} role="img" aria-label={`${label}: ${value == null ? "—" : `${v.toFixed(0)}${unit}`}`}>
      <defs>
        <radialGradient id="arc-glow" cx="50%" cy="100%" r="60%">
          <stop offset="0%" stopColor="var(--color-accent)" stopOpacity="0.35" />
          <stop offset="100%" stopColor="var(--color-accent)" stopOpacity="0" />
        </radialGradient>
      </defs>
      {/* Vệt sáng tĩnh dưới đồng hồ (giống ảnh mẫu), chỉ ở giao diện tối. */}
      <ellipse cx={CX} cy={150} rx={95} ry={45} fill="url(#arc-glow)" className="hidden [[data-theme=dark]_&]:inline" />
      <g strokeLinecap="round" strokeWidth={3.2}>
        {Array.from({ length: TICKS }, (_, i) => {
          const angle = START - (SWEEP * i) / (TICKS - 1);
          const [x1, y1] = point(angle, 70);
          const [x2, y2] = point(angle, i % 4 === 0 ? 88 : 84);
          const on = i < lit;
          return (
            <line
              key={i}
              x1={x1}
              y1={y1}
              x2={x2}
              y2={y2}
              className={on ? `${tone} stroke-current` : "stroke-black/10 dark:stroke-white/15"}
            />
          );
        })}
      </g>
      <text x={CX} y={CY - 4} textAnchor="middle" className="fill-current" fontSize={34} fontWeight={600}>
        {value == null ? "—" : v.toFixed(0)}
        <tspan fontSize={13} dx={2} dy={-14} className="opacity-70">
          {value == null ? "" : unit}
        </tspan>
      </text>
      <text x={CX} y={CY + 20} textAnchor="middle" className="fill-current opacity-60" fontSize={12}>
        {label}
      </text>
    </svg>
  );
}
