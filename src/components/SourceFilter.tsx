import { useMemo, useState } from "react";
import type { NewsEvent } from "../types";

/**
 * Filtro por canal/fonte (multi-seleção), no mesmo estilo do filtro de
 * severidade. Semântica de EXCLUSÃO: `hidden` guarda as fontes desmarcadas
 * (ocultas); vazio = nada oculto = mostra tudo. Isso evita a ambiguidade de
 * um modelo "selecionadas" (onde desmarcar a única fonte restante colapsaria
 * de volta para "mostrar tudo", o oposto do que o clique pediu).
 */
export function SourceFilter({
  events,
  hidden,
  onChange,
}: {
  events: NewsEvent[];
  /** Fontes ocultas (desmarcadas). Vazio = nenhuma oculta (mostra tudo). */
  hidden: Set<string>;
  onChange: (next: Set<string>) => void;
}) {
  const [open, setOpen] = useState(false);

  const counts = useMemo(() => {
    const c = new Map<string, number>();
    for (const e of events) c.set(e.source, (c.get(e.source) ?? 0) + 1);
    return [...c.entries()].sort((a, b) => b[1] - a[1]);
  }, [events]);

  const allShown = hidden.size === 0;
  const shownCount = counts.length - hidden.size;
  const label = allShown ? "TODAS" : `${shownCount}/${counts.length}`;

  const toggle = (source: string) => {
    const next = new Set(hidden);
    if (next.has(source)) next.delete(source);
    else next.add(source);
    onChange(next);
  };

  return (
    <div className="relative text-xs">
      <button
        onClick={() => setOpen((v) => !v)}
        className={`rounded-sm border px-2 py-0.5 font-bold tracking-wider transition-colors ${
          allShown
            ? "border-terminal-border hover:border-terminal-dim text-terminal-dim"
            : "border-terminal-amber bg-terminal-amber/10 text-terminal-amber"
        }`}
      >
        FONTE: {label} <span className="opacity-70">▾</span>
      </button>

      {open && (
        <>
          {/* Fecha ao clicar fora */}
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute left-0 top-full z-20 mt-1 max-h-72 w-64 overflow-y-auto rounded-sm border border-terminal-border bg-terminal-panel p-2 shadow-lg">
            <button
              onClick={() => onChange(new Set())}
              className={`mb-1 w-full rounded-sm px-2 py-1 text-left font-bold ${
                allShown ? "bg-terminal-amber/10 text-terminal-amber" : "text-terminal-dim hover:bg-terminal-border/40"
              }`}
            >
              MOSTRAR TODAS
            </button>
            {counts.map(([source, n]) => {
              const checked = !hidden.has(source);
              return (
                <label
                  key={source}
                  className="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 hover:bg-terminal-border/40"
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => toggle(source)}
                    className="accent-terminal-amber"
                  />
                  <span className="flex-1 truncate text-white">{source}</span>
                  <span className="text-terminal-dim">{n}</span>
                </label>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}
