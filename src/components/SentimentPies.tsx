import { useMemo } from "react";
import type { NewsEvent } from "../types";
import { sentimentShare, withinLastMinutes, type SentimentShare } from "../lib/analytics";

/** Donut SVG com as fatias compra (verde) / venda (vermelho) / neutro (cinza). */
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

function Pie({ title, s }: { title: string; s: SentimentShare }) {
  return (
    <div className="flex items-center gap-3">
      <Donut s={s} />
      <div className="text-[11px] leading-tight">
        <div className="mb-1 text-[10px] uppercase tracking-widest text-terminal-dim">
          {title} <span className="opacity-60">({s.count})</span>
        </div>
        <div className="text-terminal-green">● COMPRA {s.buy}%</div>
        <div className="text-terminal-red">● VENDA {s.sell}%</div>
        <div className="text-terminal-dim">● NEUTRO {s.neutral}%</div>
      </div>
    </div>
  );
}

/**
 * Três pizzas de sentimento (compra/venda/neutro), ponderadas por impacto:
 * Geral, últimos 30 min e últimos 5 min. Base para leitura rápida do humor
 * do mercado conforme as notícias chegam.
 */
export function SentimentPies({ events }: { events: NewsEvent[] }) {
  const now = Date.now();
  const geral = useMemo(() => sentimentShare(events), [events]);
  const m30 = useMemo(
    () => sentimentShare(withinLastMinutes(events, 30, now)),
    [events, now],
  );
  const m5 = useMemo(
    () => sentimentShare(withinLastMinutes(events, 5, now)),
    [events, now],
  );

  return (
    <section className="flex flex-wrap items-center gap-8 border-b border-terminal-border bg-terminal-panel px-4 py-3">
      <Pie title="Geral" s={geral} />
      <Pie title="30 min" s={m30} />
      <Pie title="5 min" s={m5} />
    </section>
  );
}
