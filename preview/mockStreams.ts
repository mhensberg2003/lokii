// Browser preview: Streams that move through their phases with time, so the Activity
// panel, the Downloads page and the player status can be checked without Tauri.

type Args = Record<string, unknown> | undefined;

type MockStream = {
  id: string;
  showId: number;
  episode: number;
  showTitle: string;
  source: "torbox" | "local";
  /** Milliseconds since the epoch. */
  created: number;
  watched: boolean;
};

const GB = 1024 ** 3;
const SIZE = 1.4 * GB;
/** Seconds a Local Torrent needs to download the whole file in the preview. */
const LOCAL_SECONDS = 90;
/** Seconds TorBox needs to download the Release in the preview. */
const TORBOX_SECONDS = 40;

const list: MockStream[] = [
  { id: "seed-1", showId: 154587, episode: 5, showTitle: "Frieren: Beyond Journey’s End", source: "torbox", created: Date.now() - 12_000, watched: false },
  { id: "seed-2", showId: 16498, episode: 2, showTitle: "Attack on Titan", source: "local", created: Date.now() - 30_000, watched: false },
];

type MockPhase =
  | { kind: "preparing"; step: string }
  | { kind: "downloading"; progress: number }
  | { kind: "ready" };

function phaseAt(stream: MockStream, seconds: number): MockPhase {
  if (seconds < 2) return { kind: "preparing", step: "Finding the release" };
  if (stream.source === "local") {
    return seconds < 4 ? { kind: "preparing", step: "Finding peers" } : { kind: "ready" };
  }
  if (seconds < 3) return { kind: "preparing", step: "Sending to TorBox" };
  const progress = (seconds - 3) / TORBOX_SECONDS;
  return progress < 1 ? { kind: "downloading", progress } : { kind: "ready" };
}

function view(stream: MockStream) {
  const seconds = (Date.now() - stream.created) / 1000;
  const phase = phaseAt(stream, seconds);
  const ready = phase.kind === "ready";
  const localDone = Math.min(1, Math.max(0, (seconds - 4) / LOCAL_SECONDS));
  const downloaded = stream.source === "local" ? SIZE * localDone : phase.kind === "downloading" ? SIZE * phase.progress : SIZE;
  const moving = stream.source === "local" ? ready && localDone < 1 : phase.kind === "downloading";
  return {
    id: stream.id,
    showId: stream.showId,
    episode: stream.episode,
    showTitle: seconds < 1 ? "" : stream.showTitle,
    source: stream.source,
    releaseTitle: seconds < 1 ? null : `[SubsPlease] ${stream.showTitle} - ${String(stream.episode).padStart(2, "0")} (1080p) [A1B2C3D4].mkv`,
    releaseGroup: "SubsPlease",
    fileName: ready ? `[SubsPlease] ${stream.showTitle} - ${String(stream.episode).padStart(2, "0")} (1080p).mkv` : null,
    phase,
    url: ready ? "http://127.0.0.1:0/stream/preview" : null,
    size: SIZE,
    downloaded,
    speed: moving ? 6.4 * 1024 ** 2 : 0,
    peers: moving ? 23 : 0,
    watched: stream.watched,
    startedAt: Math.floor(stream.created / 1000),
  };
}

function titleOf(showId: number, fixtures: Record<string, unknown>): string {
  const show = fixtures[`./fixtures/show-${showId}.json`] as { title?: string } | undefined;
  return show?.title ?? `Show ${showId}`;
}

export function streamHandlers(fixtures: Record<string, unknown>): Record<string, (args: Args) => unknown> {
  return {
    stream_start: (args) => {
      const showId = Number(args?.showId);
      const episode = Number(args?.episode);
      let stream = list.find((s) => s.showId === showId && s.episode === episode);
      if (!stream) {
        stream = { id: crypto.randomUUID(), showId, episode, showTitle: titleOf(showId, fixtures), source: "local", created: Date.now(), watched: false };
        list.push(stream);
      }
      return view(stream);
    },
    stream_list: () => list.map(view).sort((a, b) => b.startedAt - a.startedAt),
    stream_watched: (args) => {
      const stream = list.find((s) => s.id === args?.id);
      if (stream) stream.watched = true;
      return null;
    },
    stream_close: (args) => {
      const index = list.findIndex((s) => s.id === args?.id && s.watched);
      if (index >= 0) list.splice(index, 1);
      return null;
    },
    stream_remove: (args) => {
      const index = list.findIndex((s) => s.id === args?.id);
      if (index >= 0) list.splice(index, 1);
      return null;
    },
  };
}
