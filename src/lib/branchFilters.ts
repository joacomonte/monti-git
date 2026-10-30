// Where a feature's work stands across the environments, for the dashboard's quick filters and the card's "Shipped".
// `promotion` holds, per target (environments + base), how many of its own commits haven't reached it yet.
import type { Card } from "$lib/api";

const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);

/** Every commit already reached every target, production included: only the branch is left. */
export function isShipped(card: Card): boolean {
  return card.promotion.length > 0 && card.promotion.every(([, missing]) => missing === 0);
}

export type BranchFilter = "all" | "shipped" | "qa" | "unmerged" | "changed";

export const FILTERS: { id: BranchFilter; label: string; help: string; matches: (card: Card, base: string | null) => boolean }[] = [
  { id: "all", label: "All", help: "Every branch, in its section", matches: () => true },
  { id: "shipped", label: "Shipped", help: "All its commits are in production (and every environment): ready to clean up", matches: isShipped },
  {
    id: "qa", label: "In qa, not prod", help: "Already in an environment like qa, still missing in production: waiting for a deploy",
    matches: (card, base) =>
      card.promotion.some(([t, missing]) => t !== base && missing === 0) && card.promotion.some(([t, missing]) => t === base && missing > 0),
  },
  {
    id: "unmerged", label: "Not merged", help: "Not in any environment yet: work in progress or a pending PR",
    matches: (card) => card.promotion.length > 0 && card.promotion.every(([, missing]) => missing > 0) && !card.alreadyIn.length,
  },
  {
    id: "changed", label: "Changed after merge", help: "It was merged somewhere, but got new commits or uncommitted changes after that",
    // Partly in a target (`alreadyIn`: some of its commits are there, `promotion`: not all), merged earlier with squash/rebase
    // (`mergedBefore`), or fully in one with uncommitted work on top.
    matches: (card) =>
      card.promotion.some(([t, missing]) => missing > 0 && card.alreadyIn.includes(localName(t))) ||
      card.mergedBefore.length > 0 ||
      (card.uncommitted != null && card.promotion.some(([, missing]) => missing === 0)),
  },
];
