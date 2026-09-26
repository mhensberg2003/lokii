import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type PlayerState = {
  ready: boolean;
  pause: boolean;
  timePos: number;
  duration: number;
  buffering: boolean;
  sid: string;
  aid: string;
  title: string;
  error: string;
};

const INITIAL: PlayerState = {
  ready: false,
  pause: true,
  timePos: 0,
  duration: 0,
  buffering: false,
  sid: "",
  aid: "",
  title: "",
  error: "",
};

const PROPERTY_KEYS: Record<string, keyof PlayerState> = {
  pause: "pause",
  "time-pos": "timePos",
  duration: "duration",
  "paused-for-cache": "buffering",
  sid: "sid",
  aid: "aid",
  "media-title": "title",
};

function formatTime(seconds: number) {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

function run(command: string, args?: Record<string, unknown>) {
  return invoke(command, args).catch((err) => String(err));
}

export default function App() {
  const [state, setState] = useState<PlayerState>(INITIAL);
  const [url, setUrl] = useState("");

  useEffect(() => {
    const unlisteners = [
      listen("player://ready", () => setState((s) => ({ ...s, ready: true }))),
      listen<{ name: string; value: unknown }>("player://property", ({ payload }) => {
        const key = PROPERTY_KEYS[payload.name];
        if (!key) return;
        setState((s) => ({ ...s, [key]: payload.value ?? INITIAL[key] }));
      }),
      listen<string>("player://error", ({ payload }) => setState((s) => ({ ...s, error: payload }))),
    ];
    // mpv can start before these listeners exist, so read the current values once as well.
    invoke<Record<string, unknown>>("player_snapshot")
      .then((snapshot) =>
        setState((s) => {
          const next: PlayerState = { ...s, ready: true };
          for (const [name, value] of Object.entries(snapshot)) {
            const key = PROPERTY_KEYS[name];
            if (key) Object.assign(next, { [key]: value ?? INITIAL[key] });
          }
          return next;
        }),
      )
      .catch(() => {});
    return () => unlisteners.forEach((p) => p.then((off) => off()));
  }, []);

  const load = async () => {
    const error = await run("player_load", { url: url.trim() });
    setState((s) => ({ ...s, error: typeof error === "string" ? error : "" }));
  };

  const progress = state.duration > 0 ? (state.timePos / state.duration) * 100 : 0;

  return (
    <main className="stage">
      <header className="top">
        <span className="brand">Lokii · M0 player spike</span>
        <form
          className="source"
          onSubmit={(e) => {
            e.preventDefault();
            load();
          }}
        >
          <input
            id="source-url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="File path or http://127.0.0.1 stream URL"
            spellCheck={false}
          />
          <button type="submit" disabled={!state.ready || !url.trim()}>
            Play
          </button>
        </form>
      </header>

      {state.error && <p className="error">{state.error}</p>}
      {state.buffering && <p className="buffering">Buffering…</p>}

      <footer className="controls">
        <div className="title">{state.title || (state.ready ? "Nothing loaded" : "Starting mpv…")}</div>
        <input
          id="scrubber"
          className="scrubber"
          type="range"
          min={0}
          max={state.duration || 0}
          step={0.1}
          value={state.timePos}
          style={{ "--progress": `${progress}%` } as React.CSSProperties}
          onChange={(e) => run("player_seek", { seconds: Number(e.target.value) })}
          disabled={!state.duration}
        />
        <div className="row">
          <button onClick={() => run("player_toggle_pause")} disabled={!state.ready}>
            {state.pause ? "Play" : "Pause"}
          </button>
          <span className="time">
            {formatTime(state.timePos)} / {formatTime(state.duration)}
          </span>
          <span className="spacer" />
          <button onClick={() => run("player_cycle", { property: "sid" })} disabled={!state.ready}>
            Subtitles: {state.sid || "off"}
          </button>
          <button onClick={() => run("player_cycle", { property: "aid" })} disabled={!state.ready}>
            Audio: {state.aid || "off"}
          </button>
        </div>
      </footer>
    </main>
  );
}
