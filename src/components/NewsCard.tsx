import type { NewsEvent } from "../types";

const SENTIMENT_STYLES: Record<NewsEvent["sentiment"], string> = {
  BULLISH: "border-l-terminal-green text-terminal-green",
  BEARISH: "border-l-terminal-red text-terminal-red",
  NEUTRAL: "border-l-terminal-dim text-terminal-dim",
};

const IMPACT_BADGE: Record<NewsEvent["impact_level"], string> = {
  CRITICAL: "bg-terminal-red text-black animate-pulse",
  HIGH: "bg-terminal-amber text-black",
  MEDIUM: "bg-terminal-border text-terminal-amber",
  LOW: "bg-terminal-border text-terminal-dim",
};

function fmtTime(iso: string): string {
  const d = new Date(iso);
  return isNaN(d.getTime())
    ? "--:--:--"
    : d.toLocaleTimeString("pt-BR", { hour12: false });
}

/** Extrai um ticker curto do ativo: "S&P 500 Futuro (ES)" -> "ES". */
function tickerOf(asset: string): string {
  if (!asset) return "DIR";
  const m = asset.match(/\(([^)]+)\)/);
  if (m) return m[1].toUpperCase();
  return asset.split(/\s+/)[0].slice(0, 6).toUpperCase();
}

export function NewsCard({ ev }: { ev: NewsEvent }) {
  const sentiment = SENTIMENT_STYLES[ev.sentiment] ?? SENTIMENT_STYLES.NEUTRAL;
  const badge = IMPACT_BADGE[ev.impact_level] ?? IMPACT_BADGE.LOW;
  const { up, down } = ev.sp500_direction_probability;

  return (
    <article
      className={`border border-terminal-border border-l-4 ${sentiment} bg-terminal-panel px-4 py-3 text-sm`}
    >
      {/* Linha 1: horário / fonte / impacto / tipo de alerta */}
      <header className="flex items-center gap-3 text-xs text-terminal-dim">
        <span>{fmtTime(ev.received_at_utc)}</span>
        <span className="text-terminal-amber">[{ev.source}]</span>
        <span className={`px-2 py-px font-bold tracking-wider ${badge}`}>
          {ev.impact_level}
        </span>
        <span className="ml-auto">{ev.alert_type}</span>
      </header>

      {/* Linha 2: manchete colorida pelo sentimento */}
      <h3 className="mt-1 font-bold uppercase tracking-wide">
        {ev.sentiment === "BULLISH" ? "▲" : ev.sentiment === "BEARISH" ? "▼" : "◆"}{" "}
        {ev.event}
      </h3>

      {/* Linha 3: Atual vs Projeção vs Anterior */}
      {(ev.actual !== "N/A" || ev.forecast !== "N/A") && (
        <div className="mt-1 flex gap-6 text-xs text-gray-300">
          <span>
            ACT <b className="text-white">{ev.actual}</b>
          </span>
          <span>
            FCST <b className="text-white">{ev.forecast}</b>
          </span>
          <span>
            PREV <b className="text-white">{ev.previous}</b>
          </span>
        </div>
      )}

      {/* Linha 4: probabilidade direcional do ativo + alvo projetado */}
      <div className="mt-2 flex items-center gap-3 text-xs">
        <span className="text-terminal-dim">{tickerOf(ev.asset)}</span>
        <div className="flex h-2 flex-1 overflow-hidden rounded-sm bg-terminal-border">
          <div className="bg-terminal-green" style={{ width: `${up}%` }} />
          <div className="bg-terminal-red" style={{ width: `${down}%` }} />
        </div>
        <span className="text-terminal-green">↑{up}%</span>
        <span className="text-terminal-red">↓{down}%</span>
        <span className="ml-2 font-bold text-terminal-amber">
          {ev.projected_target_pts}
        </span>
      </div>

      {/* Linha 5: racional do Gemini */}
      <p className="mt-2 text-xs italic text-gray-400">» {ev.rationale}</p>
    </article>
  );
}
