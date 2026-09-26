import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { ArrowLeft } from "lucide-react";
import { detailLabel, sourceLabel, streams, STREAMS_KEY, upsertStream, WATCHED_AT, type StreamView } from "../lib/streams";
import { useStreams } from "../lib/useStreams";
import "./Player.css";

type PlayerState = {
  ready: boolean;
  pause: boolean;
  timePos: number;
  duration: number;
  buffering: boolean;
  sid: string;
  aid: string;
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
  error: "",
};

const PROPERTY_KEYS: Record<string, keyof PlayerState> = {
  pause: "pause",
  "time-pos": "timePos",
  duration: "duration",
  "paused-for-cache": "buffering",
  sid: "sid",
  aid: "aid",
};

function formatTime(seconds: number) {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

function run(command: string, args?: Record<string, unknown>) {
  return invoke(command, args).catch((err) => String(err));
}

/** mpv's observed properties, kept in React state. */
function usePlayerState() {
  const [state, setState] = useState<PlayerState>(INITIAL);
  useEffect(() => {
    const unlisteners = [
      listen("player://ready", () => setState((s) => ({ ...s, ready: true }))),
      listen<{ name: string; value: unknown }>("player://property", ({ payload }) => {
        const key = PROPERTY_KEYS[payload.name];
        if (key) setState((s) => ({ ...s, [key]: payload.value ?? INITIAL[key] }));
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
    return () => unlisteners.forEach((p) => p.then((off) => off()).catch(() => {}));
  }, []);
  return state;
}

/** Starts the Stream for the Episode, loads its URL into mpv, and reports Watched. */
function useStreamPlayback(showId: number, episode: number, player: PlayerState) {
  const queryClient = useQueryClient();
  const [streamId, setStreamId] = useState<string | null>(null);
  const [startError, setStartError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const stream = useStreams().data?.find((s) => s.id === streamId) ?? null;
  const loaded = useRef<string | null>(null);
  const watched = useRef(false);
  const current = useRef<string | null>(null);
  current.current = streamId;

  // Leaving the player in any way (Back, history, a route change) stops mpv and closes the Stream.
  useEffect(
    () => () => {
      run("player_stop");
      if (current.current) streams.close(current.current).catch(() => {});
    },
    [],
  );

  useEffect(() => {
    setStartError("");
    streams
      .start(showId, episode)
      .then((view) => {
        queryClient.setQueryData<StreamView[]>(STREAMS_KEY, (list) => upsertStream(list, view));
        setStreamId(view.id);
      })
      .catch((err) => setStartError(String(err)));
  }, [showId, episode, attempt, queryClient]);

  const url = stream?.phase.kind === "ready" ? stream.url : null;
  useEffect(() => {
    if (!url || !player.ready || loaded.current === url) return;
    loaded.current = url;
    run("player_load", { url });
  }, [url, player.ready]);

  useEffect(() => {
    if (!stream || watched.current || player.duration <= 0) return;
    if (player.timePos / player.duration >= WATCHED_AT) {
      watched.current = true;
      streams.watched(stream.id).catch(() => {
        watched.current = false;
      });
    }
  }, [stream, player.timePos, player.duration]);

  return { stream, startError, retry: () => setAttempt((n) => n + 1) };
}

/** The player for one Episode (`/watch/:showId/:episode`). The full player UI is issue #19. */
export function Player() {
  const params = useParams();
  const showId = Number(params.showId);
  const episode = Number(params.episode);
  const navigate = useNavigate();
  const state = usePlayerState();
  const { stream, startError, retry } = useStreamPlayback(showId, episode, state);

  const leave = () => navigate(-1);

  const playing = stream?.phase.kind === "ready";
  const title = stream?.showTitle ? `${stream.showTitle} · Episode ${episode}` : `Episode ${episode}`;

  return (
    <main className="stage">
      <header className="top">
        <button type="button" className="back" aria-label="Back" onClick={leave}>
          <ArrowLeft />
        </button>
        <div className="heading">
          <span className="title">{title}</span>
          {stream && <span className="source">{sourceLabel(stream.source)}</span>}
        </div>
      </header>

      {!playing && <StreamStatus stream={stream} startError={startError} onRetry={retry} onBack={leave} />}
      {playing && state.error && <p className="error">{state.error}</p>}
      {playing && state.buffering && <p className="buffering">Buffering…</p>}
      {playing && <Controls state={state} />}
    </main>
  );
}

type StreamStatusProps = { stream: StreamView | null; startError: string; onRetry: () => void; onBack: () => void };

/** What the Stream does before the video can play: preparing, downloading on TorBox, or failed. */
function StreamStatus({ stream, startError, onRetry, onBack }: StreamStatusProps) {
  const failed = startError !== "" || stream?.phase.kind === "failed";
  const detail = startError || (stream ? detailLabel(stream) : "Starting…");
  const progress = stream?.phase.kind === "downloading" ? stream.phase.progress : null;
  return (
    <section className="status" role="status" aria-live="polite">
      {!failed && <span className="spinner" aria-hidden="true" />}
      <p className="status-title">{failed ? "This Episode cannot play" : "Getting the Episode ready"}</p>
      <p className="status-detail">{detail}</p>
      {progress !== null && (
        <span className="status-bar" aria-hidden="true">
          <span style={{ width: `${progress * 100}%` }} />
        </span>
      )}
      {stream?.releaseTitle && <p className="status-release">{stream.releaseTitle}</p>}
      {failed && (
        <div className="status-actions">
          <button type="button" onClick={onRetry}>
            Try again
          </button>
          <button type="button" onClick={onBack}>
            Back
          </button>
        </div>
      )}
    </section>
  );
}

function Controls({ state }: { state: PlayerState }) {
  const progress = state.duration > 0 ? (state.timePos / state.duration) * 100 : 0;
  return (
    <footer className="controls">
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
  );
}
