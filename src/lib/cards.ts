// What a branch card says about where its work is, from the card facts Rust computes (branches::Status). One rule for
// the graph's cards and the dashboard.
import type { Card } from "./api";
import { localName } from "./refs";

/** "done": in the base and every environment (safe to delete). "pending": in some, still missing in others.
 *  "none": it has commits of its own and they're in none of them. null: nothing to say (no targets, or no work). */
export type MergeState = { kind: "done" | "pending" | "none"; done: string[]; missing: string[] } | null;

export function mergeState(card: Pick<Card, "promotion" | "firstParentCount">): MergeState {
  const done = card.promotion.filter(([, missing]) => missing === 0).map(([t]) => localName(t));
  const missing = card.promotion.filter(([, missing]) => missing > 0).map(([t]) => localName(t));
  if (!card.promotion.length) return null;
  if (!missing.length) return { kind: "done", done, missing };
  if (done.length) return { kind: "pending", done, missing };
  return card.firstParentCount > 0 ? { kind: "none", done, missing } : null;
}
