import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AssetResolution, RuntimeStatus } from "../types";

/**
 * Barra de controles do topo:
 *  - Ativo prioritário (ES/NQ/etc.) -> direciona as análises da IA
 *  - Preço atual do ativo -> projeta níveis de preço no gráfico
 *  - Nova API key (topo direito) -> adiciona ao pool quando uma cota esgota
 */
export function ControlBar({
  status,
  price,
  onPriceChange,
  onStatusRefresh,
}: {
  status: RuntimeStatus | null;
  price: string;
  onPriceChange: (v: string) => void;
  onStatusRefresh: () => void;
}) {
  const [assetInput, setAssetInput] = useState("");
  const [keyInput, setKeyInput] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  };

  const submitAsset = async () => {
    const asset = assetInput.trim();
    if (!asset) return;
    try {
      const r = await invoke<AssetResolution>("set_priority_asset", { asset });
      setAssetInput("");
      onStatusRefresh();
      if (r.recognized) {
        flash(`✓ ${asset.toUpperCase()} → ${r.name}`);
      } else {
        flash(`⚠ "${asset}" não reconhecido — análise genérica`);
      }
    } catch (e) {
      flash(`Erro: ${e}`);
    }
  };

  const submitKey = async () => {
    const key = keyInput.trim();
    if (!key) return;
    try {
      const total = await invoke<number>("add_api_key", { key });
      setKeyInput("");
      onStatusRefresh();
      flash(`API key adicionada (total: ${total})`);
    } catch (e) {
      flash(`Erro: ${e}`);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-3 border-b border-terminal-border bg-terminal-panel px-4 py-2 text-xs">
      {/* Ativo prioritário */}
      <label className="flex items-center gap-1 text-terminal-dim">
        ATIVO
        <input
          value={assetInput}
          onChange={(e) => setAssetInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submitAsset()}
          placeholder={status?.asset ?? "S&P 500 (ES)"}
          className="w-44 rounded-sm border border-terminal-border bg-terminal-bg px-2 py-1 text-white placeholder:text-terminal-dim focus:border-terminal-amber focus:outline-none"
        />
        <button
          onClick={submitAsset}
          className="rounded-sm border border-terminal-border px-2 py-1 hover:border-terminal-amber"
        >
          SET
        </button>
      </label>
      <span className="text-terminal-amber">» {status?.asset ?? "—"}</span>

      {/* Preço atual do ativo (projeção de níveis) */}
      <label className="flex items-center gap-1 text-terminal-dim">
        PREÇO ATUAL
        <input
          value={price}
          onChange={(e) => onPriceChange(e.target.value)}
          inputMode="decimal"
          placeholder="ex: 5600.00"
          className="w-28 rounded-sm border border-terminal-border bg-terminal-bg px-2 py-1 text-white placeholder:text-terminal-dim focus:border-terminal-amber focus:outline-none"
        />
      </label>

      {/* Nova API key — topo direito */}
      <div className="ml-auto flex items-center gap-2">
        {msg && (
          <span
            className={
              msg.startsWith("✓")
                ? "text-terminal-green"
                : "text-terminal-amber"
            }
          >
            {msg}
          </span>
        )}
        <span className="text-terminal-dim">
          KEYS: <b className="text-white">{status?.keys_count ?? 1}</b> (ativa #
          {(status?.active_key ?? 0) + 1})
        </span>
        <label className="flex items-center gap-1 text-terminal-dim">
          + API KEY
          <input
            value={keyInput}
            onChange={(e) => setKeyInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && submitKey()}
            type="password"
            placeholder="nova chave Gemini"
            className="w-48 rounded-sm border border-terminal-border bg-terminal-bg px-2 py-1 text-white placeholder:text-terminal-dim focus:border-terminal-amber focus:outline-none"
          />
          <button
            onClick={submitKey}
            className="rounded-sm border border-terminal-border px-2 py-1 hover:border-terminal-amber"
          >
            ADD
          </button>
        </label>
      </div>
    </div>
  );
}
