import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { NewsCard } from "./components/NewsCard";
import { StatusBar } from "./components/StatusBar";
import { ControlBar } from "./components/ControlBar";
import { SeverityFilter } from "./components/SeverityFilter";
import { PriceLevels } from "./components/PriceLevels";
import { SentimentPies } from "./components/SentimentPies";
import { Scoreboard } from "./components/Scoreboard";
import { useNewsStream } from "./hooks/useNewsStream";
import { registerNotificationClickFocus } from "./lib/nativeAlerts";
import type { RuntimeStatus, SeverityFilter as Filter } from "./types";

/**
 * Dashboard principal — estilo terminal Bloomberg. Tudo (feed, gráfico,
 * pizzas, contadores) é escopado pelo ATIVO ATIVO: trocar o ativo mostra
 * apenas as análises daquele papel, que são recalculadas sob a ótica dele.
 */
export default function App() {
  const { events, status, lastError } = useNewsStream();
  const [filter, setFilter] = useState<Filter>("ALL");
  const [price, setPrice] = useState("");
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);

  const refreshRuntime = () => {
    invoke<RuntimeStatus>("get_runtime_status")
      .then(setRuntime)
      .catch((e) => console.error("get_runtime_status:", e));
  };

  useEffect(() => {
    refreshRuntime();
    // Clicar na notificação nativa traz o app para frente.
    let unlisten: (() => void) | undefined;
    registerNotificationClickFocus().then((fn) => (unlisten = fn));
    return () => unlisten?.();
  }, []);

  const asset = runtime?.asset ?? "S&P 500 Futuro (ES)";

  // Escopo do dashboard: só os eventos do ativo ativo. Enquanto o runtime não
  // carregou, mostramos tudo para não piscar vazio.
  const assetEvents = useMemo(
    () => (runtime ? events.filter((e) => e.asset === asset) : events),
    [events, asset, runtime],
  );

  const counts = useMemo(() => {
    const c: Record<string, number> = {
      ALL: assetEvents.length,
      CRITICAL: 0,
      HIGH: 0,
      MEDIUM: 0,
      LOW: 0,
    };
    for (const e of assetEvents) c[e.impact_level] = (c[e.impact_level] ?? 0) + 1;
    return c;
  }, [assetEvents]);

  const visible = useMemo(
    () =>
      filter === "ALL"
        ? assetEvents
        : assetEvents.filter((e) => e.impact_level === filter),
    [assetEvents, filter],
  );

  return (
    <div className="flex h-screen flex-col bg-terminal-bg font-mono text-gray-200">
      <StatusBar status={status} eventCount={assetEvents.length} />

      <ControlBar
        status={runtime}
        price={price}
        onPriceChange={setPrice}
        onStatusRefresh={refreshRuntime}
      />

      {/* Níveis projetados (compacto, só quando há preço informado) */}
      <PriceLevels events={assetEvents} price={price} />

      {/* Pizzas de sentimento com filtro de tempo por pizza */}
      <SentimentPies events={assetEvents} />

      {/* Placar de acerto da IA (autoaprendizagem) */}
      <Scoreboard />

      {/* Filtro por severidade + fontes */}
      <div className="flex flex-wrap items-center gap-3 border-b border-terminal-border px-4 py-1.5">
        <SeverityFilter active={filter} counts={counts} onChange={setFilter} />
        <span className="ml-auto text-[10px] text-terminal-dim">
          FED · MARKETWATCH · CNBC · YAHOO FINANCE · TRUTH SOCIAL
        </span>
      </div>

      {lastError && (
        <div className="border-b border-terminal-red bg-terminal-red/10 px-4 py-2 text-xs text-terminal-red">
          ✖ {lastError}
        </div>
      )}

      {/* Feed ao vivo (filtrado por ativo + severidade) */}
      <main className="flex-1 space-y-2 overflow-y-auto p-3">
        {visible.length === 0 ? (
          <div className="mt-24 text-center text-terminal-dim">
            <p className="text-lg">
              {assetEvents.length === 0
                ? `AGUARDANDO ANÁLISES PARA ${asset}…`
                : `NENHUM EVENTO "${filter}" NO MOMENTO`}
            </p>
            <p className="mt-2 text-xs">
              {assetEvents.length === 0
                ? "Ao trocar de ativo, as notícias são re-analisadas sob a ótica dele — os cards aparecem em alguns ciclos."
                : "Ajuste o filtro de severidade acima."}
            </p>
          </div>
        ) : (
          visible.map((ev) => <NewsCard key={ev.dedup_key} ev={ev} />)
        )}
      </main>

      <footer className="border-t border-terminal-border px-4 py-1 text-[10px] text-terminal-dim">
        DADOS LOCAIS (SQLITE) · ANÁLISE GEMINI COM TRAVA ANTI-ALUCINAÇÃO · USO
        INFORMATIVO — NÃO É RECOMENDAÇÃO DE INVESTIMENTO
      </footer>
    </div>
  );
}
