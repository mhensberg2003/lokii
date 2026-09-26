import type { FranchiseEntry } from "./catalog";

export type SeasonTab = { id: number; label: string; detail: string | null };

const SEPARATORS = /^[\s:\-–—|]+/;

/**
 * Labels the Shows of a Franchise for the season strip, oldest first.
 * Uses each Show's own title minus the shared Franchise name ("Season 3 Part 2",
 * "Final Season"), so labels match what fans call them. The first Show is "Season 1".
 * Falls back to "Season N" when a title does not start with the Franchise name.
 * A label from the catalog (One Pace Arc names) wins over all of this.
 */
export function franchiseSeasons(franchise: FranchiseEntry[]): SeasonTab[] {
  const root = franchise[0]?.title.trim() ?? "";
  return franchise.map((entry, index) => ({
    id: entry.id,
    label: entry.label ?? (index === 0 ? "Season 1" : seasonName(entry.title, root) ?? `Season ${index + 1}`),
    detail: entry.seasonYear ? String(entry.seasonYear) : null,
  }));
}

function seasonName(title: string, root: string): string | null {
  if (!root || !title.toLowerCase().startsWith(root.toLowerCase())) return null;
  const rest = title.slice(root.length).replace(SEPARATORS, "").trim();
  return rest || null;
}

export type EpisodeRange = { id: string; label: string; start: number; end: number };

/** Splits long Shows into pages of episodes: "1–50", "51–100", … Returns [] when one page is enough. */
export function episodeRanges(total: number, size = 50): EpisodeRange[] {
  if (total <= size) return [];
  const ranges: EpisodeRange[] = [];
  for (let start = 1; start <= total; start += size) {
    const end = Math.min(start + size - 1, total);
    ranges.push({ id: `${start}-${end}`, label: `${start}–${end}`, start, end });
  }
  return ranges;
}
