import type { SeverityFilter as Filter } from "../types";

const LEVELS: Array<{ key: Filter; label: string; color: string }> = [
  { key: "ALL", label: "TODOS", color: "text-white" },
  { key: "CRITICAL", label: "CRITICAL", color: "text-terminal-red" },
  { key: "HIGH", label: "HIGH", color: "text-terminal-amber" },
  { key: "MEDIUM", label: "MEDIUM", color: "text-terminal-amber" },
  { key: "LOW", label: "LOW", color: "text-terminal-dim" },
];

/**
 * Chips clicáveis de filtro por severidade. Clicar em CRITICAL mostra só os
 * CRITICAL; TODOS mostra tudo.
 */
export function SeverityFilter({
  active,
  counts,
  onChange,
}: {
  active: Filter;
  counts: Record<string, number>;
  onChange: (f: Filter) => void;
}) {
  return (
    <div className="flex items-center gap-2 text-xs">
      {LEVELS.map(({ key, label, color }) => {
        const isActive = active === key;
        const n = key === "ALL" ? counts.ALL : counts[key] ?? 0;
        return (
          <button
            key={key}
            onClick={() => onChange(key)}
            className={`rounded-sm border px-2 py-0.5 font-bold tracking-wider transition-colors ${
              isActive
                ? "border-terminal-amber bg-terminal-amber/10"
                : "border-terminal-border hover:border-terminal-dim"
            } ${color}`}
          >
            {label} <span className="opacity-70">{n}</span>
          </button>
        );
      })}
    </div>
  );
}
