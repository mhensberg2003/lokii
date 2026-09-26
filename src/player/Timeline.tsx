import { useRef, useState, type PointerEvent, type RefObject } from "react";
import { formatTime, type SkipSegment } from "../lib/player";
import styles from "./Player.module.css";

type Props = {
  time: number;
  duration: number;
  segments: SkipSegment[];
  onSeek: (seconds: number) => void;
};

const percent = (value: number, duration: number) => `${Math.min(100, Math.max(0, (value / duration) * 100))}%`;

/** Pointer handling: hover shows a time, dragging previews, release seeks. */
function useScrub(track: RefObject<HTMLDivElement | null>, duration: number, onSeek: (seconds: number) => void) {
  const [hover, setHover] = useState<number | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const known = duration > 0;

  const secondsAt = (event: PointerEvent) => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0;
    return Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)) * duration;
  };

  const handlers = {
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (!known || event.button !== 0) return;
      event.currentTarget.setPointerCapture(event.pointerId);
      setDrag(secondsAt(event));
    },
    onPointerMove: (event: PointerEvent) => {
      if (!known) return;
      const seconds = secondsAt(event);
      setHover(seconds);
      if (drag !== null) setDrag(seconds);
    },
    onPointerUp: (event: PointerEvent) => {
      if (drag === null) return;
      onSeek(secondsAt(event));
      setDrag(null);
    },
    onPointerLeave: () => setHover(null),
    onPointerCancel: () => setDrag(null),
    onLostPointerCapture: () => setDrag(null),
  };
  return { hover, drag, handlers };
}

/**
 * The thin timeline. Skip Segments show as lighter bands. Dragging previews the position
 * and seeks once on release, so a torrent Stream does not get a request per pixel.
 */
export function Timeline({ time, duration, segments, onSeek }: Props) {
  const track = useRef<HTMLDivElement>(null);
  const { hover, drag, handlers } = useScrub(track, duration, onSeek);
  const known = duration > 0;

  const shown = drag ?? time;
  const label = drag ?? hover;
  return (
    <div
      ref={track}
      className={styles.timeline}
      data-dragging={drag !== null || undefined}
      role="slider"
      aria-label="Position"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(shown)}
      aria-valuetext={`${formatTime(shown)} of ${formatTime(duration)}`}
      {...handlers}
    >
      <div className={styles.rail}>
        {known &&
          segments.map((s) => (
            <span
              key={s.kind}
              className={styles.segment}
              style={{ left: percent(s.start, duration), width: percent(s.end - s.start, duration) }}
            />
          ))}
        <span className={styles.played} style={{ width: known ? percent(shown, duration) : "0%" }} />
      </div>
      {known && <span className={styles.knob} style={{ left: percent(shown, duration) }} />}
      {known && label !== null && (
        <span className={styles.tooltip} style={{ left: percent(label, duration) }}>
          {formatTime(label)}
        </span>
      )}
    </div>
  );
}
