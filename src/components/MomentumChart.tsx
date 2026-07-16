import { useMemo } from "react";
import type { NewsEvent } from "../types";
import { aggregate, bucketByMinute, projectPrice } from "../lib/analytics";

/**
 * Painel "compilado do momento": consolida TODOS os eventos numa leitura
 * única — viés líquido em pontos, probabilidade direcional ponderada por
 * impacto, contagem de sentimento, mini-gráfico de momentum por minuto e,
 * se houver preço informado, os níveis de preço projetados.
 */
export function MomentumChart({
  events,
  price,
  asset,
}: {
  events: NewsEvent[];
  price: string;
  asset: string;
}) {
  const agg = useMemo(() => aggregate(events), [events]);
  const buckets = useMemo(() => bucketByMinute(events, 20), [events]);

  const priceNum = parseFloat(price.replace(",", "."));
  const proj =
    isFinite(priceNum) && priceNum > 0 ? projectPrice(priceNum, agg) : null;

  const maxAbs = Math.max(1, ...buckets.map((b) => Math.abs(b.pts)));
  const biasColor =
    agg.netPts > 0
      ? "text-terminal-green"
      : agg.netPts < 0
        ? "text-terminal-red"
        : "text-terminal-dim";

  // Geometria do mini-gráfico (barras a partir da linha central).
  const W = 460;
  const H = 90;
  const mid = H / 2;
  const bw = W / buckets.length;

  return (
    <section className="border-b border-terminal-border bg-terminal-panel px-4 py-3">
      <div className="flex flex-wrap items-stretch gap-6">
        {/* Bloco 1: viés + probabilidade */}
        <div className="min-w-[190px]">
          <div className="text-[10px] uppercase tracking-widest text-terminal-dim">
            Viés líquido — {asset}
          </div>
          <div className={`font-mono text-3xl font-bold ${biasColor}`}>
            {agg.netPts > 0 ? "+" : ""}
            {agg.netPts} pts
          </div>
          <div className="mt-2 flex h-2 w-full overflow-hidden rounded-sm bg-terminal-border">
            <div className="bg-terminal-green" style={{ width: `${agg.probUp}%` }} />
            <div className="bg-terminal-red" style={{ width: `${agg.probDown}%` }} />
          </div>
          <div className="mt-1 flex justify-between text-[11px]">
            <span className="text-terminal-green">▲ {agg.probUp}%</span>
            <span className="text-terminal-dim">{agg.count} eventos</span>
            <span className="text-terminal-red">▼ {agg.probDown}%</span>
          </div>
          <div className="mt-1 flex gap-3 text-[11px]">
            <span className="text-terminal-green">BULL {agg.bull}</span>
            <span className="text-terminal-red">BEAR {agg.bear}</span>
            <span className="text-terminal-dim">NEU {agg.neu}</span>
          </div>
        </div>

        {/* Bloco 2: momentum por minuto */}
        <div className="flex-1">
          <div className="text-[10px] uppercase tracking-widest text-terminal-dim">
            Momentum (pts/min · últimos 20m)
          </div>
          <svg
            viewBox={`0 0 ${W} ${H}`}
            className="mt-1 h-[90px] w-full"
            preserveAspectRatio="none"
          >
            <line x1={0} y1={mid} x2={W} y2={mid} stroke="#1f2733" strokeWidth={1} />
            {buckets.map((b, i) => {
              const h = (Math.abs(b.pts) / maxAbs) * (mid - 4);
              const up = b.pts >= 0;
              return (
                <rect
                  key={i}
                  x={i * bw + 1}
                  y={up ? mid - h : mid}
                  width={Math.max(1, bw - 2)}
                  height={Math.max(0, h)}
                  fill={b.pts === 0 ? "#1f2733" : up ? "#00d26a" : "#ff3b3b"}
                />
              );
            })}
          </svg>
        </div>

        {/* Bloco 3: níveis de preço projetados (se houver preço) */}
        <div className="min-w-[170px] border-l border-terminal-border pl-6">
          <div className="text-[10px] uppercase tracking-widest text-terminal-dim">
            Níveis projetados
          </div>
          {proj ? (
            <div className="mt-1 space-y-1 font-mono text-sm">
              <div className="flex justify-between gap-4">
                <span className="text-terminal-dim">ALVO</span>
                <b className={biasColor}>{proj.target.toFixed(2)}</b>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-terminal-dim">ALTA</span>
                <span className="text-terminal-green">{proj.up.toFixed(2)}</span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-terminal-dim">BASE</span>
                <span className="text-white">{proj.base.toFixed(2)}</span>
              </div>
              <div className="flex justify-between gap-4">
                <span className="text-terminal-dim">BAIXA</span>
                <span className="text-terminal-red">{proj.down.toFixed(2)}</span>
              </div>
            </div>
          ) : (
            <div className="mt-2 text-[11px] leading-relaxed text-terminal-dim">
              Informe o PREÇO ATUAL no topo para projetar os níveis
              (base ± viés líquido).
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
