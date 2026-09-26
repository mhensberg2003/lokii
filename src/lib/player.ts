// Player data contract between the Rust core (src-tauri/src/player, src-tauri/src/skip)
// and the UI, plus the pure rules of the player screen. Terms follow CONTEXT.md.

import { invoke } from "@tauri-apps/api/core";
import type { ShowDetails } from "./catalog";

/** mpv's observed properties, as the UI keeps them. */
export type PlayerState = {
  ready: boolean;
  pause: boolean;
  timePos: number;
  duration: number;
  buffering: boolean;
  sid: string;
  aid: string;
  volume: number;
  mute: boolean;
  error: string;
};

export const INITIAL_PLAYER: PlayerState = {
  ready: false,
  pause: true,
  timePos: 0,
  duration: 0,
  buffering: false,
  sid: "",
  aid: "",
  volume: 100,
  mute: false,
  error: "",
};

/** mpv property name → PlayerState key. */
export const PROPERTY_KEYS: Record<string, keyof PlayerState> = {
  pause: "pause",
  "time-pos": "timePos",
  duration: "duration",
  "paused-for-cache": "buffering",
  sid: "sid",
  aid: "aid",
  volume: "volume",
  mute: "mute",
};

export type TrackKind = "audio" | "sub";

export type Track = {
  id: number;
  kind: TrackKind;
  title: string | null;
  lang: string | null;
  codec: string | null;
  selected: boolean;
};

export type SegmentKind = "intro" | "outro" | "recap";

export type SkipSegment = { kind: SegmentKind; start: number; end: number };

/** Seconds that the arrow keys and the skip buttons move. */
export const SEEK_STEP = 10;
/** Volume change of the up and down arrow keys, in percent. */
export const VOLUME_STEP = 5;
/** Milliseconds without mouse movement before the controls hide. */
export const IDLE_AFTER = 3000;
/** Seconds before the end at which Next Episode shows when AniSkip has no outro. */
export const NEXT_EPISODE_LEAD = 60;

export const player = {
  load: (url: string) => invoke<void>("player_load", { url }),
  stop: () => invoke<void>("player_stop"),
  snapshot: () => invoke<Record<string, unknown>>("player_snapshot"),
  togglePause: () => invoke<void>("player_toggle_pause"),
  seek: (seconds: number) => invoke<void>("player_seek", { seconds }),
  seekBy: (seconds: number) => invoke<void>("player_seek_by", { seconds }),
  setVolume: (volume: number) => invoke<void>("player_set_volume", { volume }),
  toggleMute: () => invoke<void>("player_toggle_mute"),
  cycle: (property: "sid" | "aid") => invoke<void>("player_cycle", { property }),
  tracks: () => invoke<Track[]>("player_tracks"),
  setTrack: (kind: TrackKind, id: number | null) => invoke<void>("player_set_track", { kind, id }),
  skipSegments: (showId: number, episode: number, duration: number) =>
    invoke<SkipSegment[]>("skip_segments", { showId, episode, duration }),
};

/** "4:07", or "1:02:09" from one hour. */
export function formatTime(seconds: number): string {
  const s = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rest = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${rest}` : `${m}:${rest}`;
}

/** The Skip Segment that contains the time, if any. */
export function activeSegment(segments: SkipSegment[], time: number): SkipSegment | null {
  return segments.find((s) => time >= s.start && time < s.end) ?? null;
}

export function skipLabel(kind: SegmentKind): string {
  return kind === "intro" ? "Skip Intro" : kind === "outro" ? "Skip Outro" : "Skip Recap";
}

export type NextEpisode = { showId: number; episode: number; title: string | null; showTitle: string | null };

/**
 * The Episode after this one: the next aired Episode of the Show, else Episode 1 of the
 * next TV Show in the Franchise. Null at the end of the Franchise, or when the next Show has not aired.
 */
export function nextEpisodeOf(show: ShowDetails, episode: number): NextEpisode | null {
  const next = show.episodeList.find((e) => e.number === episode + 1);
  if (next) {
    return next.airingAt === null ? { showId: show.id, episode: next.number, title: next.title, showTitle: null } : null;
  }
  const index = show.franchise.findIndex((entry) => entry.id === show.id);
  const sequel = index < 0 ? undefined : show.franchise.slice(index + 1).find((entry) => isSeries(entry.format));
  if (!sequel || sequel.status === "NOT_YET_RELEASED" || sequel.status === null) return null;
  return { showId: sequel.id, episode: 1, title: null, showTitle: sequel.title };
}

function isSeries(format: string | null): boolean {
  return format === "TV" || format === "TV_SHORT" || format === "ONA";
}

/** Next Episode shows from the start of the outro, else in the last minute. */
export function nextEpisodeDue(time: number, duration: number, segments: SkipSegment[]): boolean {
  if (duration <= 0) return false;
  const outro = segments.find((s) => s.kind === "outro");
  if (outro && time >= outro.start) return true;
  return duration - time <= NEXT_EPISODE_LEAD;
}

export type ShortcutAction =
  | { kind: "togglePause" }
  | { kind: "seekBy"; seconds: number }
  | { kind: "volumeBy"; amount: number }
  | { kind: "fullscreen" }
  | { kind: "mute" }
  | { kind: "cycle"; property: "sid" | "aid" }
  | { kind: "escape" };

/** The action of a key on the player screen (issue #21), or null. */
export function shortcutFor(key: string): ShortcutAction | null {
  switch (key) {
    case " ":
    case "k":
      return { kind: "togglePause" };
    case "ArrowLeft":
      return { kind: "seekBy", seconds: -SEEK_STEP };
    case "ArrowRight":
      return { kind: "seekBy", seconds: SEEK_STEP };
    case "ArrowUp":
      return { kind: "volumeBy", amount: VOLUME_STEP };
    case "ArrowDown":
      return { kind: "volumeBy", amount: -VOLUME_STEP };
    case "f":
      return { kind: "fullscreen" };
    case "m":
      return { kind: "mute" };
    case "s":
      return { kind: "cycle", property: "sid" };
    case "a":
      return { kind: "cycle", property: "aid" };
    case "Escape":
      return { kind: "escape" };
    default:
      return null;
  }
}

/** ISO 639-2 codes that files use, mapped to the codes Intl.DisplayNames knows. */
const LANGUAGE_CODES: Record<string, string> = {
  jpn: "ja", eng: "en", ger: "de", deu: "de", fre: "fr", fra: "fr", spa: "es", ita: "it",
  por: "pt", rus: "ru", ara: "ar", chi: "zh", zho: "zh", kor: "ko", pol: "pl", tur: "tr",
  ind: "id", tha: "th", vie: "vi", may: "ms", msa: "ms", hin: "hi", heb: "he", dut: "nl",
  nld: "nl", swe: "sv", ukr: "uk", cze: "cs", ces: "cs", hun: "hu", gre: "el", ell: "el",
};

/** "Japanese" for "jpn" or "ja"; null when the code is missing or unknown. */
export function languageName(code: string | null): string | null {
  if (!code || code === "und") return null;
  const tag = LANGUAGE_CODES[code.toLowerCase()] ?? code;
  try {
    const name = new Intl.DisplayNames(["en"], { type: "language" }).of(tag);
    return name && name !== tag ? name : code;
  } catch {
    return code;
  }
}

/** The main line of a track in the panel, and an optional second line. */
export function trackLabel(track: Track, position: number): { name: string; detail: string | null } {
  const language = languageName(track.lang);
  const title = track.title?.trim() || null;
  const name = language ?? title ?? `Track ${position}`;
  const detail = title && title !== name ? title : null;
  return { name, detail };
}
