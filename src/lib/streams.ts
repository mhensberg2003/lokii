// Stream data contract between the Rust core (src-tauri/src/stream) and the UI.
// Terms follow CONTEXT.md: Stream, Source, TorBox, Local Torrent, Episode, Watched.

import { invoke } from "@tauri-apps/api/core";
import { sizeLabel } from "./releases";

export type SourceKind = "torbox" | "local";

export type Phase =
  | { kind: "preparing"; step: string }
  | { kind: "downloading"; progress: number }
  | { kind: "ready" }
  | { kind: "failed"; message: string };

export type StreamView = {
  id: string;
  showId: number;
  episode: number;
  showTitle: string;
  source: SourceKind;
  releaseTitle: string | null;
  releaseGroup: string | null;
  /** The file of the Episode inside the Release. */
  fileName: string | null;
  phase: Phase;
  /** The HTTP URL mpv plays, once the phase is "ready". */
  url: string | null;
  /** File size in bytes; 0 until known. */
  size: number;
  downloaded: number;
  /** Bytes per second. */
  speed: number;
  peers: number;
  watched: boolean;
  /** Unix seconds. */
  startedAt: number;
};

export const streams = {
  start: (showId: number, episode: number) => invoke<StreamView>("stream_start", { showId, episode }),
  list: () => invoke<StreamView[]>("stream_list"),
  /** The player passed 90% of the Episode. */
  watched: (id: string) => invoke<void>("stream_watched", { id }),
  /** The player left the Stream. */
  close: (id: string) => invoke<void>("stream_close", { id }),
  /** Stops the Stream and deletes its local data. */
  remove: (id: string) => invoke<void>("stream_remove", { id }),
};

export const STREAMS_KEY = ["streams"] as const;

/** An Episode is Watched after 90% of its length (CONTEXT.md). */
export const WATCHED_AT = 0.9;

/** 0–1, or null when the size is not known yet. */
export function downloadFraction(stream: StreamView): number | null {
  if (stream.phase.kind === "downloading") return stream.phase.progress;
  if (stream.size <= 0) return null;
  return Math.min(1, stream.downloaded / stream.size);
}

/** The short status in the Activity panel: "Preparing", "42%", "Ready", "Failed". */
export function statusLabel(stream: StreamView): string {
  switch (stream.phase.kind) {
    case "preparing":
      return "Preparing";
    case "failed":
      return "Failed";
    case "downloading":
      return `${Math.floor(stream.phase.progress * 100)}%`;
    case "ready": {
      const fraction = downloadFraction(stream);
      return stream.source === "local" && fraction !== null && fraction < 1 ? `${Math.floor(fraction * 100)}%` : "Ready";
    }
  }
}

/** The longer line under the title: what the Stream does now. */
export function detailLabel(stream: StreamView): string {
  switch (stream.phase.kind) {
    case "preparing":
      return `${stream.phase.step}…`;
    case "failed":
      return stream.phase.message;
    case "downloading":
      return "TorBox is downloading the release";
    case "ready":
      if (stream.source === "torbox") return "Ready on TorBox";
      return (downloadFraction(stream) ?? 0) >= 1 ? "Downloaded" : "Downloading from peers";
  }
}

/** "5.2 MB/s", or null when nothing moves. */
export function speedLabel(bytesPerSecond: number): string | null {
  if (bytesPerSecond <= 0) return null;
  const mb = bytesPerSecond / 1024 ** 2;
  return mb >= 1 ? `${mb.toFixed(1)} MB/s` : `${Math.max(1, Math.round(bytesPerSecond / 1024))} KB/s`;
}

/** "1.2 GB of 1.4 GB", or the size alone when nothing is downloaded. */
export function progressLabel(stream: StreamView): string | null {
  const total = sizeLabel(stream.size);
  if (!total) return null;
  const done = sizeLabel(stream.downloaded);
  return done && stream.downloaded < stream.size ? `${done} of ${total}` : total;
}

export function sourceLabel(source: SourceKind): string {
  return source === "torbox" ? "TorBox" : "Local Torrent";
}

/** True while the Stream still changes on its own (the list refreshes during this time). */
export function isActive(stream: StreamView): boolean {
  if (stream.phase.kind === "preparing" || stream.phase.kind === "downloading") return true;
  return stream.phase.kind === "ready" && stream.source === "local" && stream.downloaded < stream.size;
}

/** Inserts or replaces a Stream, newest first. */
export function upsertStream(list: StreamView[] | undefined, stream: StreamView): StreamView[] {
  const others = (list ?? []).filter((s) => s.id !== stream.id);
  return [stream, ...others].sort((a, b) => b.startedAt - a.startedAt);
}
