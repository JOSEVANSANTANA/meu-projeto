import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AddFeedResult } from "../types";

/**
 * Gerenciador de fontes RSS: cole a URL (site ou feed) e o backend valida +
 * autodescobre o feed. As fontes ficam salvas (settings.json) e alimentam o
 * mesmo pipeline de análise. Cada fonte passa pelo filtro de relevância macro.
 */
export function FeedManager() {
  const [feeds, setFeeds] = useState<string[]>([]);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 4000);
  };

  const refresh = () =>
    invoke<string[]>("get_rss_feeds").then(setFeeds).catch(() => {});

  useEffect(() => {
    refresh();
  }, []);

  const add = async () => {
    const u = url.trim();
    if (!u) return;
    setBusy(true);
    try {
      const r = await invoke<AddFeedResult>("add_rss_feed", { url: u });
      setUrl("");
      await refresh();
      flash(`✓ Fonte adicionada (${r.count} itens): ${r.url}`);
    } catch (e) {
      flash(`✖ ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const remove = async (u: string) => {
    try {
      const list = await invoke<string[]>("remove_rss_feed", { url: u });
      setFeeds(list);
    } catch (e) {
      flash(`✖ ${e}`);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-terminal-border bg-terminal-panel px-4 py-1.5 text-xs">
      <span className="text-[10px] uppercase tracking-widest text-terminal-amber">
        Fontes RSS
      </span>
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && !busy && add()}
        placeholder="cole a URL (ex.: https://finance.yahoo.com/)"
        className="w-72 rounded-sm border border-terminal-border bg-terminal-bg px-2 py-1 text-white placeholder:text-terminal-dim focus:border-terminal-amber focus:outline-none"
      />
      <button
        onClick={add}
        disabled={busy}
        className="rounded-sm border border-terminal-border px-2 py-1 hover:border-terminal-amber disabled:opacity-50"
      >
        {busy ? "VERIFICANDO…" : "ADD"}
      </button>

      {/* Chips das fontes adicionadas, com remover */}
      {feeds.map((f) => (
        <span
          key={f}
          className="flex items-center gap-1 rounded-sm border border-terminal-border px-2 py-0.5 text-terminal-dim"
          title={f}
        >
          {hostOf(f)}
          <button
            onClick={() => remove(f)}
            className="text-terminal-red hover:text-white"
            title="Remover"
          >
            ✕
          </button>
        </span>
      ))}

      {msg && (
        <span
          className={`ml-auto ${msg.startsWith("✓") ? "text-terminal-green" : "text-terminal-red"}`}
        >
          {msg}
        </span>
      )}
    </div>
  );
}

function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}
