// Browser preview: a simulated mpv. It plays a 24-minute Episode in real time and sends
// the same `player://` events as the Rust player, so the player screen can be checked
// without Tauri.

type Args = Record<string, unknown> | undefined;
type Emit = (event: string, payload: unknown) => void;

const DURATION = 1440;
const TICK_MS = 250;

const TRACKS = [
  { id: 1, kind: "audio", title: null, lang: "jpn", codec: "aac" },
  { id: 2, kind: "audio", title: "English Dub", lang: "eng", codec: "aac" },
  { id: 1, kind: "sub", title: "Full Subtitles [SubsPlease]", lang: "eng", codec: "ass" },
  { id: 2, kind: "sub", title: "Signs & Songs", lang: "eng", codec: "ass" },
  { id: 3, kind: "sub", title: null, lang: "spa", codec: "ass" },
];

export function playerHandlers(emit: Emit): Record<string, (args: Args) => unknown> {
  const s = { loaded: false, pause: true, time: 0, volume: 100, mute: false, sid: "1", aid: "1" };
  let timer: number | undefined;
  const property = (name: string, value: unknown) => emit("player://property", { name, value });

  const seek = (seconds: number) => {
    s.time = Math.min(DURATION, Math.max(0, seconds));
    property("time-pos", s.time);
  };
  const setPause = (pause: boolean) => {
    s.pause = pause;
    property("pause", pause);
  };
  const tick = () => {
    if (s.loaded && !s.pause) seek(s.time + TICK_MS / 1000);
  };
  const cycle = (key: "sid" | "aid") => {
    const ids = TRACKS.filter((t) => t.kind === (key === "sid" ? "sub" : "audio")).map((t) => String(t.id));
    const order = key === "sid" ? [...ids, "no"] : ids;
    s[key] = order[(order.indexOf(s[key]) + 1) % order.length];
    property(key, s[key]);
  };

  return {
    player_snapshot: () => ({
      pause: s.pause,
      "time-pos": s.loaded ? s.time : null,
      duration: s.loaded ? DURATION : null,
      "paused-for-cache": false,
      sid: s.sid,
      aid: s.aid,
      volume: s.volume,
      mute: s.mute,
    }),
    player_load: () => {
      s.loaded = true;
      s.time = 0;
      emit("player://file-loaded", null);
      property("duration", DURATION);
      setPause(false);
      timer ??= window.setInterval(tick, TICK_MS);
      return null;
    },
    player_stop: () => {
      s.loaded = false;
      setPause(true);
      return null;
    },
    player_toggle_pause: () => setPause(!s.pause),
    player_seek: (args) => seek(Number(args?.seconds)),
    player_seek_by: (args) => seek(s.time + Number(args?.seconds)),
    player_set_volume: (args) => {
      s.volume = Math.min(100, Math.max(0, Number(args?.volume)));
      s.mute = false;
      property("volume", s.volume);
      property("mute", false);
    },
    player_toggle_mute: () => {
      s.mute = !s.mute;
      property("mute", s.mute);
    },
    player_cycle: (args) => cycle(args?.property === "aid" ? "aid" : "sid"),
    player_tracks: () =>
      s.loaded ? TRACKS.map((t) => ({ ...t, selected: String(t.id) === (t.kind === "sub" ? s.sid : s.aid) })) : [],
    player_set_track: (args) => {
      const key = args?.kind === "audio" ? "aid" : "sid";
      s[key] = args?.id == null ? "no" : String(args.id);
      property(key, s[key]);
    },
    skip_segments: () => [
      { kind: "intro", start: 90, end: 180 },
      { kind: "outro", start: 1340, end: 1430 },
    ],
    "plugin:window|is_fullscreen": () => false,
    "plugin:window|set_fullscreen": () => null,
  };
}
