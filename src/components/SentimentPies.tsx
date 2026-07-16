import { useMemo, useState } from "react";
import type { NewsEvent } from "../types";
import { sentimentShare, withinLastMinutes, type SentimentShare } from "../lib/analytics";

/** Opções de janela de tempo (0 = Geral / todos os eventos). */
const TIMEFRAMES: Array<{ label: string; minutes: number }> = [
  { label: "Geral", minutes: 0 },
  { label: "1 min", minutes: 1 },
  { label: "5 min", minutes: 5 },
  { label: "30 min", minutes: 30 },
  { label: "1 hora", minutes: 60 },
  { label: "4 horas", minutes: 240 },
];

/** Donut SVG: compra (verde) / venda (vermelho) / neutro (cinza). */
function Donut({ s }: { s: SentimentShare }) {
  const r = 26;
  const c = 2 * Math.PI * r;
  const segs = [
    { v: s.buy, color: "#00d26a" },
    { v: s.sell, color: "#ff3b3b" },
    { v: s.neutral, color: "#8b98a9" },
  ];
  let offset = 0;
  const empty = s.count === 0;
  return (
    <svg viewBox="0 0 72 72" className="h-20 w-20">
      <g transform="rotate(-90 36 36)">
        <circle cx={36} cy={36} r={r} fill="none" stroke="#1f2733" strokeWidth={10} />
        {!empty &&
          segs.map((seg, i) => {
            const len = (seg.v / 100) * c;
            const el = (
              <circle
                key={i}
                cx={36}
                cy={36}
                r={r}
                fill="none"
                stroke={seg.color}
                strokeWidth={10}
                strokeDasharray={`${len} ${c - len}`}
                strokeDashoffset={-offset}
              />
            );
            offset += len;
            return el;
          })}
      </g>
    </svg>
  );
}

/** Uma pizza com seu próprio seletor de janela de tempo. */
function Pie({ events, initial, now }: { events: NewsEvent[]; initial: number; now: number }) {
  const [minutes, setMinutes] = useState(initial);
  const scoped = useMemo(
    () => (minutes === 0 ? events : withinLastMinutes(events, minutes, now)),
    [events, minutes, now],
  );
  const s = sentimentShare(scoped);

  return (
    <div className="flex items-center gap-3">
      <Donut s={s} />
      <div className="text-[11px] leading-tight">
        <select
          value={minutes}
          onChange={(e) => setMinutes(Number(e.target.value))}
          className="mb-1 rounded-sm border border-terminal-border bg-terminal-bg px-1 py-0.5 text-[10px] uppercase tracking-widest text-terminal-amber focus:border-terminal-amber focus:outline-none"
        >
          {TIMEFRAMES.map((t) => (
            <option key={t.minutes} value={t.minutes}>
              {t.label}
            </option>
          ))}
        </select>
        <span className="ml-1 text-terminal-dim">({s.count})</span>
        <div className="text-terminal-green">● COMPRA {s.buy}%</div>
        <div className="text-terminal-red">● VENDA {s.sell}%</div>
        <div className="text-terminal-dim">● NEUTRO {s.neutral}%</div>
      </div>
    </div>
  );
}

/**
 * Três pizzas de sentimento (compra/venda/neutro), ponderadas por impacto.
 * Cada uma tem seletor próprio de janela (Geral/1m/5m/30m/1h/4h) — você pode,
 * por exemplo, deixar as três em 5 min. Padrão: Geral / 30 min / 5 min.
 */
export function SentimentPies({ events }: { events: NewsEvent[] }) {
  const now = Date.now();
  return (
    <section className="flex flex-wrap items-center gap-8 border-b border-terminal-border bg-terminal-panel px-4 py-3">
      <Pie events={events} initial={0} now={now} />
      <Pie events={events} initial={30} now={now} />
      <Pie events={events} initial={5} now={now} />
    </section>
  );
}
