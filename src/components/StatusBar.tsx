import { useEffect, useState } from "react";
import type { EngineStatus } from "../types";

const STATUS_STYLES: Record<EngineStatus, { dot: string; label: string }> = {
  ONLINE: { dot: "bg-terminal-green", label: "ENGINE ONLINE" },
  CONNECTING: { dot: "bg-terminal-amber animate-pulse", label: "CONNECTING…" },
  ERROR: { dot: "bg-terminal-red animate-pulse", label: "ENGINE ERROR" },
};

export function StatusBar({
  status,
  eventCount,
}: {
  status: EngineStatus;
  eventCount: number;
}) {
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(t);
  }, []);

  const s = STATUS_STYLES[status];
  const utc = now.toISOString().slice(11, 19);
  const local = now.toLocaleTimeString("pt-BR", { hour12: false });

  return (
    <header className="flex items-center gap-4 border-b border-terminal-border bg-terminal-panel px-4 py-2 text-xs">
      <span className="text-base font-bold tracking-widest text-terminal-amber">
        ESF NEWS MONITOR
      </span>
      <span className="text-terminal-dim">S&P 500 FUTURES · GEMINI AI</span>

      <span className="ml-auto flex items-center gap-2">
        <i className={`inline-block h-2 w-2 rounded-full ${s.dot}`} />
        {s.label}
      </span>
      <span className="text-terminal-dim">EVENTS: {eventCount}</span>
      <span className="text-terminal-dim">UTC {utc}</span>
      <span className="text-white">LOCAL {local}</span>
    </header>
  );
}
