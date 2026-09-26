import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { ArrowLeft } from "lucide-react";
import { catalog } from "../lib/catalog";
import {
  activeSegment,
  nextEpisodeDue,
  nextEpisodeOf,
  player,
  type NextEpisode,
  type PlayerState,
  type ShortcutAction,
  type SkipSegment,
  type Track,
} from "../lib/player";
import { sourceLabel, type StreamView } from "../lib/streams";
import { Controls, IconButton } from "../player/Controls";
import { NextEpisodeCard, SkipButton } from "../player/Prompts";
import { StreamStatus } from "../player/StreamStatus";
import { TrackPanel } from "../player/TrackPanel";
import { usePlayerState, useSkipSegments, useTracks } from "../player/usePlayer";
import { useFullscreen, useIdle, useShortcuts } from "../player/useScreen";
import { useStreamPlayback } from "../player/useStreamPlayback";
import { useWatchProgress } from "../player/useWatchProgress";
import styles from "../player/Player.module.css";

type Screen = ReturnType<typeof useFullscreen>;

/** The player (`/watch/:showId/:episode`). Fullscreen stays on across Episodes. */
export function Player() {
  const params = useParams();
  const showId = Number(params.showId);
  const episode = Number(params.episode);
  const screen = useFullscreen();
  // A new key per Episode, so Next Episode starts with a new Stream and a clean state.
  return <EpisodePlayer key={`${showId}/${episode}`} showId={showId} episode={episode} screen={screen} />;
}

function useEpisode(showId: number, episode: number) {
  const show = useQuery({ queryKey: ["catalog", "show", showId], queryFn: () => catalog.show(showId) }).data;
  const title = show?.episodeList.find((e) => e.number === episode)?.title ?? null;
  return { show, title, next: show ? nextEpisodeOf(show, episode) : null };
}

type ShortcutContext = { volume: number; screen: Screen; panelOpen: boolean; closePanel: () => void };

function runShortcut(action: ShortcutAction, ctx: ShortcutContext) {
  const ignore = (promise: Promise<unknown>) => void promise.catch(() => {});
  switch (action.kind) {
    case "togglePause":
      return ignore(player.togglePause());
    case "seekBy":
      return ignore(player.seekBy(action.seconds));
    case "volumeBy":
      return ignore(player.setVolume(ctx.volume + action.amount));
    case "mute":
      return ignore(player.toggleMute());
    case "cycle":
      return ignore(player.cycle(action.property));
    case "fullscreen":
      return ctx.screen.setFullscreen(!ctx.screen.fullscreen);
    case "escape":
      if (ctx.panelOpen) ctx.closePanel();
      else if (ctx.screen.fullscreen) ctx.screen.setFullscreen(false);
  }
}

type EpisodePlayerProps = { showId: number; episode: number; screen: Screen };

function EpisodePlayer({ showId, episode, screen }: EpisodePlayerProps) {
  const navigate = useNavigate();
  const mpv = usePlayerState();
  const { stream, fileLoaded, startError, retry, markWatched } = useStreamPlayback(showId, episode, mpv);
  // Before this Episode's file loads, mpv can still report the previous file's time.
  const state = fileLoaded ? mpv : { ...mpv, timePos: 0, duration: 0 };
  const progress = useWatchProgress(showId, episode, state, fileLoaded);
  // Next Episode can show from the outro, before 90%: leaving with it counts as Watched.
  const finish = () => Promise.all([progress.markWatched(state.duration), markWatched()]).then(() => {});
  const { show, title, next } = useEpisode(showId, episode);
  const segments = useSkipSegments(showId, episode, state.duration);
  const tracks = useTracks(state.sid, state.aid);
  const [panelOpen, setPanelOpen] = useState(false);
  const [onControls, setOnControls] = useState(false);
  const playing = stream?.phase.kind === "ready";
  const { idle, poke } = useIdle(!playing || state.pause || panelOpen || onControls);
  const closePanel = () => setPanelOpen(false);
  useShortcuts((action) => {
    poke();
    runShortcut(action, { volume: state.volume, screen, panelOpen, closePanel });
  });

  const hover = { onPointerEnter: () => setOnControls(true), onPointerLeave: () => setOnControls(false) };
  const onSurfaceClick = () => (panelOpen ? closePanel() : playing && void player.togglePause().catch(() => {}));
  const layer = { state, segments, tracks, next, screen, panelOpen, hover, finish };
  return (
    <main className={styles.stage} data-idle={idle || undefined} data-fullscreen={screen.fullscreen || undefined}>
      <div
        className={styles.surface}
        onClick={onSurfaceClick}
        onDoubleClick={() => screen.setFullscreen(!screen.fullscreen)}
      />
      <Heading showTitle={stream?.showTitle || show?.title || ""} episode={episode} title={title} stream={stream} onBack={() => navigate(-1)} hover={hover} />
      {!playing && <StreamStatus stream={stream} startError={startError} onRetry={retry} onBack={() => navigate(-1)} />}
      {playing && <PlaybackLayer {...layer} onTogglePanel={() => setPanelOpen((open) => !open)} />}
    </main>
  );
}

type Hover = { onPointerEnter: () => void; onPointerLeave: () => void };

type HeadingProps = {
  showTitle: string;
  episode: number;
  title: string | null;
  stream: StreamView | null;
  onBack: () => void;
  hover: Hover;
};

function Heading({ showTitle, episode, title, stream, onBack, hover }: HeadingProps) {
  const detail = [`Episode ${episode}`, title, stream && sourceLabel(stream.source)].filter(Boolean).join(" · ");
  return (
    <header className={styles.top} {...hover}>
      <IconButton label="Back" onClick={onBack}>
        <ArrowLeft />
      </IconButton>
      <div className={styles.heading}>
        <span className={styles.title}>{showTitle || `Episode ${episode}`}</span>
        <span className={styles.subtitle}>{detail}</span>
      </div>
    </header>
  );
}

type LayerProps = {
  state: PlayerState;
  segments: SkipSegment[];
  tracks: Track[];
  next: NextEpisode | null;
  screen: Screen;
  panelOpen: boolean;
  hover: Hover;
  onTogglePanel: () => void;
  /** Marks this Episode Watched before Next Episode. */
  finish: () => Promise<void>;
};

/** Everything over the playing video: notices, Skip and Next Episode, the panel, the controls. */
function PlaybackLayer({ state, segments, tracks, next, screen, panelOpen, hover, onTogglePanel, finish }: LayerProps) {
  const navigate = useNavigate();
  const [nextHidden, setNextHidden] = useState(false);
  const segment = activeSegment(segments, state.timePos);
  const showNext = next !== null && !nextHidden && nextEpisodeDue(state.timePos, state.duration, segments);
  const playNext = () => {
    if (!next) return;
    void finish().then(() => navigate(`/watch/${next.showId}/${next.episode}`, { replace: true }));
  };

  return (
    <>
      {state.error && <p className={styles.notice}>{state.error}</p>}
      {!state.error && state.buffering && <span className={styles.buffering} role="status" aria-label="Buffering" />}
      <div className={styles.bottom} {...hover}>
        <div className={styles.prompts} hidden={panelOpen}>
          {showNext && next && <NextEpisodeCard next={next} onPlay={playNext} onDismiss={() => setNextHidden(true)} />}
          {!showNext && segment && <SkipButton segment={segment} onSkip={() => void player.seek(segment.end).catch(() => {})} />}
        </div>
        {panelOpen && <TrackPanel tracks={tracks} />}
        <Controls
          state={state}
          segments={segments}
          fullscreen={screen.fullscreen}
          panelOpen={panelOpen}
          onTogglePanel={onTogglePanel}
          onFullscreen={() => screen.setFullscreen(!screen.fullscreen)}
        />
      </div>
    </>
  );
}
