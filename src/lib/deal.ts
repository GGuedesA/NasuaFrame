import type { Intent } from "#lib/api.ts";

// depth 0 at the limit, 1 for a free buy or a sell at double the limit
export function deal(intent: Intent, limit: number, price: number) {
  const buy = intent === "buy";
  const depth = buy ? (limit ? 1 - price / limit : 0) : price ? 1 - limit / price : 0;
  const off = limit ? Math.round((Math.abs(price - limit) / limit) * 100) : 0;
  return {
    depth: Math.min(Math.max(depth, 0), 1),
    label: !limit ? "" : off ? `${buy ? "-" : "+"}${off}%` : "0%",
  };
}
