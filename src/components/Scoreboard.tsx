import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AccuracyStats } from "../types";

/**
 * Placar de acerto da IA (autoaprendizagem). Mostra a taxa de acerto de
 * DIREÇÃO das predições contra o movimento REAL do mercado (Yahoo Finance),
 * o viés de magnitude e o acerto por nível de impacto. O backend realimenta
 * esses números no prompt do Gemini para ele se calibrar sozinho.
 */
export function Scoreboard() {
  const [stats, setStats] = useState<AccuracyStats | null>(null);

  useEffect(() => {
    const load = () =>
      invoke<AccuracyStats>("get_accuracy").then(setStats).catch(() => {});
    load();
    const t = setInterval(load, 30_000);
    let un: (() => void) | undefined;
    listen<AccuracyStats>("accuracy-updated", ({ payload }) => setStats(payload)).then(
      (fn) => (un = fn),
    );
    return () => {
      clearInterval(t);
      un?.();
    };
  }, []);

  const scored = stats?.scored ?? 0;
  const rateColor =
    !stats || scored === 0
      ? "text-terminal-dim"
      : stats.hit_rate >= 55
        ? "text-terminal-green"
        : stats.hit_rate >= 45
          ? "text-terminal-amber"
          : "text-terminal-red";

  const mag = stats?.magnitude_factor ?? 1;
  const magText =
    scored === 0
      ? "—"
      : mag > 1.15
        ? `subestima ${mag.toFixed(2)}x`
        : mag < 0.85
          ? `superestima ${mag.toFixed(2)}x`
          : "calibrada";

  return (
    <div className="flex flex-wrap items-center gap-5 border-b border-terminal-border bg-terminal-panel px-4 py-1.5 text-xs">
      <span className="text-[10px] uppercase tracking-widest text-terminal-amber">
        Placar IA
      </span>
      {scored === 0 ? (
        <span className="text-terminal-dim">
          coletando resultados… (as predições são pontuadas ~20 min após,
          contra o preço real)
        </span>
      ) : (
        <>
          <span>
            ACERTO DE DIREÇÃO <b className={rateColor}>{stats!.hit_rate}%</b>
          </span>
          <span className="text-terminal-dim">
            {stats!.hits}/{scored} predições
          </span>
          <span>
            MAGNITUDE <b className="text-white">{magText}</b>
          </span>
          <span className="flex gap-2 text-terminal-dim">
            {stats!.by_impact
              .filter((i) => i.total > 0)
              .map((i) => (
                <span key={i.impact_level}>
                  {i.impact_level} {i.hits}/{i.total}
                </span>
              ))}
          </span>
        </>
      )}
    </div>
  );
}
