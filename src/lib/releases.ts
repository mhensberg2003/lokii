// Index data contract between the Rust core (src-tauri/src/index) and the UI.
// Terms follow CONTEXT.md: Release, Batch, Best Release, Chosen Release.

import { invoke } from "@tauri-apps/api/core";

/** Which Episodes of the Show a Release contains. */
export type Coverage =
  | { kind: "episode"; episode: number }
  | { kind: "range"; first: number; last: number }
  | { kind: "show" };

export type Release = {
  /** Lower-case hex info hash. Identifies the Release. */
  infoHash: string;
  title: string;
  group: string | null;
  /** Vertical resolution: 1080, 720, 2160. */
  resolution: number | null;
  sizeBytes: number;
  seeders: number;
  leechers: number;
  fileCount: number;
  coverage: Coverage;
  /** SeaDex recommends this Release for the Show. */
  isBest: boolean;
  /** Unix seconds. */
  publishedAt: number;
  magnet: string;
  /** The Release page on AnimeTosho. */
  link: string;
};

export type EpisodeReleases = {
  showId: number;
  episode: number;
  /** Best Release first, then by resolution and seeders. */
  releases: Release[];
  /** Info hash of the Chosen Release, or null when no Release can play the Episode. */
  chosen: string | null;
  /** True when the Chosen Release comes from the user's pick. */
  pickedByUser: boolean;
};

export const releases = {
  forEpisode: (showId: number, episode: number) => invoke<EpisodeReleases>("index_releases", { showId, episode }),
  /** Picks a Release for the Show. `null` returns to the automatic rule. */
  pick: (showId: number, episode: number, infoHash: string | null) =>
    invoke<EpisodeReleases>("index_pick", { showId, episode, infoHash }),
};

export function releasesKey(showId: number, episode: number) {
  return ["index", "releases", showId, episode] as const;
}

export function chosenRelease(data: EpisodeReleases): Release | null {
  return data.releases.find((release) => release.infoHash === data.chosen) ?? null;
}

/** 2160 → "4K", 1080 → "1080p". */
export function resolutionLabel(resolution: number | null): string | null {
  if (resolution === null) return null;
  return resolution >= 2160 ? "4K" : `${resolution}p`;
}

/** Bytes as "820 MB" or "45.3 GB". */
export function sizeLabel(bytes: number): string | null {
  if (bytes <= 0) return null;
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb >= 100 ? Math.round(gb) : gb.toFixed(1)} GB`;
  return `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

/** "Batch" for a full Show, "Batch 1–6" for a range, null for one Episode. */
export function batchLabel(coverage: Coverage): string | null {
  if (coverage.kind === "show") return "Batch";
  if (coverage.kind === "range") return `Batch ${coverage.first}–${coverage.last}`;
  return null;
}

/** The distinct resolutions in a list, highest first. */
export function resolutionsIn(list: Release[]): number[] {
  const set = new Set(list.map((release) => release.resolution).filter((r): r is number => r !== null));
  return [...set].sort((a, b) => b - a);
}
