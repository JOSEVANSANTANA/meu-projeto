import type { NewsEvent } from "../types";

/** Peso de cada nível de impacto no cálculo agregado (viés do momento). */
export const IMPACT_WEIGHT: Record<NewsEvent["impact_level"], number> = {
  CRITICAL: 3,
  HIGH: 2,
  MEDIUM: 1,
  LOW: 0.5,
};

/**
 * Extrai o número de "projected_target_pts" preservando o sinal.
 *  "+15 pts" -> 15 ; "-25 pts" -> -25 ; "0 pts" / "N/A" -> 0
 */
export function parsePts(s: string | undefined): number {
  if (!s) return 0;
  const m = s.replace(",", ".").match(/-?\d+(?:\.\d+)?/);
  return m ? parseFloat(m[0]) : 0;
}

export interface Aggregate {
  count: number;
  netPts: number; // soma dos pts projetados (viés direcional consolidado)
  probUp: number; // probabilidade média ponderada por impacto
  probDown: number;
  bull: number;
  bear: number;
  neu: number;
}

/** Compila todos os eventos num retrato do momento. */
export function aggregate(events: NewsEvent[]): Aggregate {
  let netPts = 0;
  let wSum = 0;
  let wUp = 0;
  let bull = 0;
  let bear = 0;
  let neu = 0;

  for (const e of events) {
    const w = IMPACT_WEIGHT[e.impact_level] ?? 0.5;
    netPts += parsePts(e.projected_target_pts);
    wSum += w;
    wUp += w * (e.sp500_direction_probability?.up ?? 50);
    if (e.sentiment === "BULLISH") bull++;
    else if (e.sentiment === "BEARISH") bear++;
    else neu++;
  }

  const probUp = wSum > 0 ? Math.round(wUp / wSum) : 50;
  return {
    count: events.length,
    netPts: Math.round(netPts * 10) / 10,
    probUp,
    probDown: 100 - probUp,
    bull,
    bear,
    neu,
  };
}

export interface Bucket {
  label: string; // HH:MM
  pts: number; // soma de pts projetados naquele minuto
}

/**
 * Agrupa os eventos por minuto nos últimos `buckets` minutos, somando os pts
 * projetados — base do mini-gráfico de momentum.
 */
export function bucketByMinute(
  events: NewsEvent[],
  buckets = 20,
  now: number = Date.now(),
): Bucket[] {
  const out: Bucket[] = Array.from({ length: buckets }, (_, i) => {
    const d = new Date(now - (buckets - 1 - i) * 60_000);
    const label = `${String(d.getUTCHours()).padStart(2, "0")}:${String(
      d.getUTCMinutes(),
    ).padStart(2, "0")}`;
    return { label, pts: 0 };
  });

  for (const e of events) {
    const ts = new Date(e.received_at_utc).getTime();
    if (isNaN(ts)) continue;
    const minAgo = Math.floor((now - ts) / 60_000);
    if (minAgo >= 0 && minAgo < buckets) {
      out[buckets - 1 - minAgo].pts += parsePts(e.projected_target_pts);
    }
  }
  return out;
}

/** Níveis de preço projetados a partir de um preço atual informado. */
export interface PriceProjection {
  base: number;
  target: number; // base + netPts
  up: number; // cenário de alta
  down: number; // cenário de baixa
}

export function projectPrice(price: number, agg: Aggregate): PriceProjection {
  const span = Math.max(Math.abs(agg.netPts), 5); // banda mínima de 5 pts
  return {
    base: price,
    target: Math.round((price + agg.netPts) * 100) / 100,
    up: Math.round((price + span) * 100) / 100,
    down: Math.round((price - span) * 100) / 100,
  };
}
