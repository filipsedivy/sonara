// Summary statistics for the measurements (from the ADR 0009 spike,
// stats.ts). Percentiles interpolate linearly between the closest
// ranks (NumPy's default method), so p95 of 100 samples lies between the 95th and 96th values.

export interface Summary {
  n: number;
  min: number;
  mean: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

export function percentile(sorted: ArrayLike<number>, p: number): number {
  if (sorted.length === 0) return Number.NaN;
  const rank = (p / 100) * (sorted.length - 1);
  const lo = Math.floor(rank);
  const hi = Math.ceil(rank);
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo);
}

export function summarize(values: ArrayLike<number>): Summary {
  const sorted = Float64Array.from(values).sort();
  let sum = 0;
  for (const v of sorted) sum += v;
  const n = sorted.length;
  return {
    n,
    min: n ? sorted[0] : Number.NaN,
    mean: n ? sum / n : Number.NaN,
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    p99: percentile(sorted, 99),
    max: n ? sorted[n - 1] : Number.NaN,
  };
}

/** Rounds every number of a summary for the report. */
export function rounded(s: Summary, digits = 3): Summary {
  const f = 10 ** digits;
  const r = (x: number) => Math.round(x * f) / f;
  return { n: s.n, min: r(s.min), mean: r(s.mean), p50: r(s.p50), p95: r(s.p95), p99: r(s.p99), max: r(s.max) };
}

export const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
