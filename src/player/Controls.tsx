import type { ReactNode } from "react";
import { Captions, Maximize, Minimize, Pause, Play, RotateCcw, RotateCw, Volume1, Volume2, VolumeX } from "lucide-react";
import { formatTime, player, SEEK_STEP, type PlayerState, type SkipSegment } from "../lib/player";
import { Timeline } from "./Timeline";
import styles from "./Player.module.css";

type IconButtonProps = {
  label: string;
  /** The key that does the same, shown in the tooltip. */
  shortcut?: string;
  pressed?: boolean;
  onClick: () => void;
  children: ReactNode;
};

export function IconButton({ label, shortcut, pressed, onClick, children }: IconButtonProps) {
  return (
    <button
      type="button"
      className={styles.iconButton}
      aria-label={label}
      aria-pressed={pressed}
      title={shortcut ? `${label} (${shortcut})` : label}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function ignore(promise: Promise<unknown>) {
  promise.catch(() => {});
}

function Volume({ volume, mute }: { volume: number; mute: boolean }) {
  const level = mute ? 0 : volume;
  const Icon = level === 0 ? VolumeX : level < 50 ? Volume1 : Volume2;
  return (
    <div className={styles.volume}>
      <IconButton label={mute ? "Unmute" : "Mute"} shortcut="M" onClick={() => ignore(player.toggleMute())}>
        <Icon />
      </IconButton>
      <input
        className={styles.volumeSlider}
        type="range"
        min={0}
        max={100}
        step={1}
        value={level}
        aria-label="Volume"
        style={{ "--level": `${level}%` } as React.CSSProperties}
        onChange={(e) => ignore(player.setVolume(Number(e.target.value)))}
      />
    </div>
  );
}

type ControlsProps = {
  state: PlayerState;
  segments: SkipSegment[];
  fullscreen: boolean;
  panelOpen: boolean;
  onTogglePanel: () => void;
  onFullscreen: () => void;
};

/** The bottom bar: timeline, then play, skip back and forward, volume, time, tracks, fullscreen. */
export function Controls({ state, segments, fullscreen, panelOpen, onTogglePanel, onFullscreen }: ControlsProps) {
  return (
    <div className={styles.controls}>
      <Timeline time={state.timePos} duration={state.duration} segments={segments} onSeek={(s) => ignore(player.seek(s))} />
      <div className={styles.row}>
        <IconButton label={state.pause ? "Play" : "Pause"} shortcut="Space" onClick={() => ignore(player.togglePause())}>
          {state.pause ? <Play /> : <Pause />}
        </IconButton>
        <IconButton label={`Back ${SEEK_STEP} seconds`} shortcut="←" onClick={() => ignore(player.seekBy(-SEEK_STEP))}>
          <RotateCcw />
          <span className={styles.stepLabel}>{SEEK_STEP}</span>
        </IconButton>
        <IconButton label={`Forward ${SEEK_STEP} seconds`} shortcut="→" onClick={() => ignore(player.seekBy(SEEK_STEP))}>
          <RotateCw />
          <span className={styles.stepLabel}>{SEEK_STEP}</span>
        </IconButton>
        <Volume volume={state.volume} mute={state.mute} />
        <span className={styles.time}>
          {formatTime(state.timePos)} <span className={styles.timeTotal}>/ {formatTime(state.duration)}</span>
        </span>
        <span className={styles.spacer} />
        <IconButton label="Subtitles and audio" pressed={panelOpen} onClick={onTogglePanel}>
          <Captions />
        </IconButton>
        <IconButton label={fullscreen ? "Exit full screen" : "Full screen"} shortcut="F" onClick={onFullscreen}>
          {fullscreen ? <Minimize /> : <Maximize />}
        </IconButton>
      </div>
    </div>
  );
}
