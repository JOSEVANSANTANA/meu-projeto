import { useMemo } from "react";
import type { NewsEvent } from "../types";
import { aggregate, projectPrice } from "../lib/analytics";

/**
 * Faixa compacta de NÍVEIS PROJETADOS: aparece só quando você informa o PREÇO
 * ATUAL. Projeta base ± viés líquido (soma dos pts das notícias do ativo).
 * BASE = seu preço; ALVO = base + viés; ALTA/BAIXA = cenários (banda mín. 5 pts).
 */
export function PriceLevels({ events, price }: { events: NewsEvent[]; price: string }) {
  const agg = useMemo(() => aggregate(events), [events]);
  const priceNum = parseFloat(price.replace(",", "."));
  if (!isFinite(priceNum) || priceNum <= 0) return null;

  const proj = projectPrice(priceNum, agg);
  const biasColor =
    agg.netPts > 0
      ? "text-terminal-green"
      : agg.netPts < 0
        ? "text-terminal-red"
        : "text-terminal-dim";

  const Cell = ({ label, value, cls }: { label: string; value: string; cls: string }) => (
    <span className="flex items-baseline gap-1">
      <span className="text-[10px] uppercase tracking-widest text-terminal-dim">{label}</span>
      <b className={`font-mono ${cls}`}>{value}</b>
    </span>
  );

  return (
    <div className="flex flex-wrap items-center gap-6 border-b border-terminal-border bg-terminal-panel px-4 py-1.5 text-sm">
      <span className="text-[10px] uppercase tracking-widest text-terminal-amber">
        Níveis Projetados
      </span>
      <Cell label="Base" value={proj.base.toFixed(2)} cls="text-white" />
      <Cell
        label="Viés"
        value={`${agg.netPts > 0 ? "+" : ""}${agg.netPts} pts`}
        cls={biasColor}
      />
      <Cell label="Alvo" value={proj.target.toFixed(2)} cls={biasColor} />
      <Cell label="Alta" value={proj.up.toFixed(2)} cls="text-terminal-green" />
      <Cell label="Baixa" value={proj.down.toFixed(2)} cls="text-terminal-red" />
    </div>
  );
}
