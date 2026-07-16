import { NewsCard } from "./components/NewsCard";
import { StatusBar } from "./components/StatusBar";
import { useNewsStream } from "./hooks/useNewsStream";

/**
 * Dashboard principal — estilo terminal Bloomberg:
 * fundo escuro, fonte monoespaçada, feed ao vivo colorido por sentimento.
 */
export default function App() {
  const { events, status, lastError } = useNewsStream();

  const critical = events.filter((e) => e.impact_level === "CRITICAL").length;
  const bullish = events.filter((e) => e.sentiment === "BULLISH").length;
  const bearish = events.filter((e) => e.sentiment === "BEARISH").length;

  return (
    <div className="flex h-screen flex-col bg-terminal-bg font-mono text-gray-200">
      <StatusBar status={status} eventCount={events.length} />

      {/* Régua de métricas da sessão */}
      <div className="flex gap-6 border-b border-terminal-border px-4 py-1.5 text-xs">
        <span>
          CRITICAL <b className="text-terminal-red">{critical}</b>
        </span>
        <span>
          BULLISH <b className="text-terminal-green">{bullish}</b>
        </span>
        <span>
          BEARISH <b className="text-terminal-red">{bearish}</b>
        </span>
        <span className="ml-auto text-terminal-dim">
          FONTES: INVESTING.COM · FINANCIALJUICE · TRUTH SOCIAL
        </span>
      </div>

      {lastError && (
        <div className="border-b border-terminal-red bg-terminal-red/10 px-4 py-2 text-xs text-terminal-red">
          ✖ {lastError}
        </div>
      )}

      {/* Feed ao vivo */}
      <main className="flex-1 space-y-2 overflow-y-auto p-3">
        {events.length === 0 ? (
          <div className="mt-24 text-center text-terminal-dim">
            <p className="text-lg">AGUARDANDO FLUXO DE NOTÍCIAS…</p>
            <p className="mt-2 text-xs">
              O motor Rust está varrendo o calendário econômico e os feeds.
              <br />
              Apenas eventos de alto impacto (3★, CPI, Payroll, FOMC) aparecem aqui.
            </p>
          </div>
        ) : (
          events.map((ev) => <NewsCard key={ev.dedup_key} ev={ev} />)
        )}
      </main>

      <footer className="border-t border-terminal-border px-4 py-1 text-[10px] text-terminal-dim">
        DADOS LOCAIS (SQLITE) · ANÁLISE GEMINI · USO INFORMATIVO — NÃO É
        RECOMENDAÇÃO DE INVESTIMENTO
      </footer>
    </div>
  );
}
