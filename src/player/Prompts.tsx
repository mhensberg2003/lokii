import { ChevronsRight, Play, X } from "lucide-react";
import { skipLabel, type NextEpisode, type SkipSegment } from "../lib/player";
import styles from "./Player.module.css";

/** Shows during a Skip Segment; jumps to its end. */
export function SkipButton({ segment, onSkip }: { segment: SkipSegment; onSkip: () => void }) {
  return (
    <button type="button" className={styles.skip} onClick={onSkip}>
      {skipLabel(segment.kind)}
      <ChevronsRight />
    </button>
  );
}

type NextProps = { next: NextEpisode; onPlay: () => void; onDismiss: () => void };

/** Shows near the end of the Episode. */
export function NextEpisodeCard({ next, onPlay, onDismiss }: NextProps) {
  const heading = next.showTitle ? `${next.showTitle} · Episode ${next.episode}` : `Episode ${next.episode}`;
  return (
    <section className={styles.nextCard} aria-label="Next Episode">
      <div className={styles.nextText}>
        <span className={styles.nextLabel}>Next Episode</span>
        <span className={styles.nextTitle}>{heading}</span>
        {next.title && <span className={styles.nextDetail}>{next.title}</span>}
      </div>
      <div className={styles.nextActions}>
        <button type="button" className={styles.nextPlay} onClick={onPlay}>
          <Play />
          Play
        </button>
        <button type="button" className={styles.nextDismiss} aria-label="Hide Next Episode" onClick={onDismiss}>
          <X />
        </button>
      </div>
    </section>
  );
}
