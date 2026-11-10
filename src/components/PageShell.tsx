import type { ReactNode } from "react";

interface Props {
  title: string;
  subtitle?: string;
  children?: ReactNode;
}

export default function PageShell({ title, subtitle, children }: Props) {
  return (
    <section className="mx-auto max-w-5xl">
      <h1 className="text-2xl font-semibold">{title}</h1>
      {subtitle && <p className="mt-1 text-sm opacity-60">{subtitle}</p>}
      <div className="mt-6">{children}</div>
    </section>
  );
}
