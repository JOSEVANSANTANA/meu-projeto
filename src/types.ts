/** Espelho TypeScript do struct `NewsEvent` do backend Rust (serde flatten). */
export interface NewsEvent {
  id: number;
  dedup_key: string;
  received_at_utc: string;
  asset: string;
  source: string;
  event: string;
  impact_level: "CRITICAL" | "HIGH" | "MEDIUM" | "LOW";
  actual: string;
  forecast: string;
  previous: string;
  sentiment: "BULLISH" | "BEARISH" | "NEUTRAL";
  sp500_direction_probability: { up: number; down: number };
  projected_target_pts: string;
  rationale: string;
  alert_type: "HIGH_VOLATILITY" | "TREND_CONFIRMATION" | "REVERSAL_RISK" | "INFO";
}

export type EngineStatus = "CONNECTING" | "ONLINE" | "ERROR";

/** Estado de configuração em runtime, vindo do backend. */
export interface RuntimeStatus {
  keys_count: number;
  active_key: number;
  asset: string;
}

export type ImpactLevel = NewsEvent["impact_level"];
/** Filtro de severidade do feed ("ALL" = todos). */
export type SeverityFilter = "ALL" | ImpactLevel;

/** Resultado da resolução de um ativo (código ou nome) no backend. */
export interface AssetResolution {
  code: string;
  name: string;
  recognized: boolean;
}
