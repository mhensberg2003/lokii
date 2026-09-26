// Library data contract between the Rust core (src-tauri/src/library) and the UI:
// Watch Progress, Up Next, Continue watching and the Watchlist. Terms follow CONTEXT.md.

import { invoke } from "@tauri-apps/api/core";
import type { ShowCard } from "./catalog";

export type WatchProgress = {
  showId: number;
  episode: number;
  /** Seconds. */
  position: number;
  /** Seconds. */
  duration: number;
  watched: boolean;
  /** Unix milliseconds. */
  updatedAt: number;
};

export type UpNext = {
  showId: number;
  episode: number;
  /** Seconds to resume from; 0 starts the Episode. */
  position: number;
  duration: number;
};

export type ShowLibrary = {
  progress: WatchProgress[];
  upNext: UpNext | null;
  onWatchlist: boolean;
};

export type ContinueItem = {
  show: ShowCard;
  episode: number;
  episodeTitle: string | null;
  thumbnailUrl: string | null;
  position: number;
  duration: number;
};

export const library = {
  /** Returns true when the Episode is Watched. */
  saveProgress: (showId: number, episode: number, position: number, duration: number) =>
    invoke<boolean>("library_save_progress", { showId, episode, position, duration }),
  show: (showId: number) => invoke<ShowLibrary>("library_show", { showId }),
  continueWatching: () => invoke<ContinueItem[]>("library_continue"),
  watchlist: () => invoke<ShowCard[]>("library_watchlist"),
  setWatchlist: (showId: number, on: boolean) => invoke<void>("library_set_watchlist", { showId, on }),
};

export const LIBRARY_KEY = ["library"] as const;
export const libraryKeys = {
  show: (showId: number) => [...LIBRARY_KEY, "show", showId] as const,
  continueWatching: [...LIBRARY_KEY, "continue"] as const,
  watchlist: [...LIBRARY_KEY, "watchlist"] as const,
};

/** The player saves Watch Progress after this many seconds of playback. */
export const SAVE_EVERY = 5;
/** Positions before this many seconds are not saved, and do not resume. */
export const RESUME_MIN = 10;

/** The position to resume an Episode from, or null to start it from the beginning. */
export function resumeFrom(progress: WatchProgress | undefined): number | null {
  if (!progress || progress.watched || progress.position < RESUME_MIN) return null;
  return progress.position;
}

/** 0–1 for the bar under an Episode: full when Watched. */
export function progressFraction(progress: Pick<WatchProgress, "position" | "duration" | "watched">): number {
  if (progress.watched) return 1;
  return progress.duration > 0 ? Math.min(1, progress.position / progress.duration) : 0;
}

/** "12 min left" for an Episode to resume, "Up next" for one to start. */
export function continueLabel(item: Pick<ContinueItem, "position" | "duration">): string {
  if (item.position <= 0 || item.duration <= 0) return "Up next";
  const minutes = Math.max(1, Math.round((item.duration - item.position) / 60));
  return `${minutes} min left`;
}

/** The Play button of a Show page: resume or start the Up Next Episode of this Show. */
export function playAction(showId: number, upNext: UpNext | null, fallback: number | null) {
  if (upNext && upNext.showId === showId) {
    const resume = upNext.position >= RESUME_MIN;
    return { episode: upNext.episode, label: `${resume ? "Resume" : "Play"} episode ${upNext.episode}` };
  }
  return fallback === null ? null : { episode: fallback, label: `Play episode ${fallback}` };
}
