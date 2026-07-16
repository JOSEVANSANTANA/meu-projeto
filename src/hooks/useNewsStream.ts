import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EngineStatus, NewsEvent } from "../types";
import { requestAttention } from "../lib/nativeAlerts";

const MAX_FEED_SIZE = 200;

/** Beep de alerta via Web Audio — dois tons curtos, estilo squawk box. */
function playAlertSound(critical: boolean) {
  const ctx = new AudioContext();
  const beep = (freq: number, start: number, dur: number) => {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "square";
    osc.frequency.value = freq;
    gain.gain.setValueAtTime(0.15, ctx.currentTime + start);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + start + dur);
    osc.connect(gain).connect(ctx.destination);
    osc.start(ctx.currentTime + start);
    osc.stop(ctx.currentTime + start + dur);
  };
  beep(critical ? 1200 : 880, 0, 0.15);
  beep(critical ? 1200 : 880, 0.2, 0.15);
  if (critical) beep(1600, 0.4, 0.25);
}

/**
 * Hook central do dashboard:
 *  1. Hidrata o feed com o histórico do SQLite (command `get_recent_events`)
 *  2. Assina o evento Tauri "news-event" emitido pelo loop Rust
 *  3. Dispara o som de alerta para CRITICAL / HIGH
 */
export function useNewsStream() {
  const [events, setEvents] = useState<NewsEvent[]>([]);
  const [status, setStatus] = useState<EngineStatus>("CONNECTING");
  const [lastError, setLastError] = useState<string | null>(null);
  const seen = useRef(new Set<string>());

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      // 1. Histórico local. Se o command responde, o backend Rust está
      //    vivo -> marcamos ONLINE (o evento "engine-status" emitido no
      //    startup pode ter ocorrido antes deste listener existir).
      try {
        const history = await invoke<NewsEvent[]>("get_recent_events", { limit: 100 });
        if (!disposed) {
          history.forEach((e) => seen.current.add(e.dedup_key));
          setEvents(history);
          setStatus("ONLINE");
        }
      } catch (e) {
        console.error("Falha ao carregar histórico:", e);
      }

      // 2. Stream ao vivo vindo do Rust
      unlisteners.push(
        await listen<NewsEvent>("news-event", ({ payload }) => {
          if (seen.current.has(payload.dedup_key)) return;
          seen.current.add(payload.dedup_key);
          setEvents((prev) => [payload, ...prev].slice(0, MAX_FEED_SIZE));

          // 3. Alerta sonoro + flash na barra de tarefas (a notificação nativa
          //    é disparada pelo Rust; clicar nela traz o app para frente).
          if (payload.impact_level === "CRITICAL" || payload.impact_level === "HIGH") {
            playAlertSound(payload.impact_level === "CRITICAL");
            void requestAttention();
          }
        }),
        await listen<string>("engine-status", ({ payload }) => {
          setStatus(payload === "ONLINE" ? "ONLINE" : "CONNECTING");
        }),
        await listen<string>("engine-error", ({ payload }) => {
          setStatus("ERROR");
          setLastError(payload);
        }),
      );
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return { events, status, lastError };
}
