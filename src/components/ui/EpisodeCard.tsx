import { Play } from "lucide-react";
import type { EpisodeInfo } from "../../lib/catalog";
import { airingLabel } from "../../lib/format";
import { Artwork } from "./Artwork";
import styles from "./EpisodeCard.module.css";

type EpisodeCardProps = {
  episode: EpisodeInfo;
  /** Used when the Episode has no thumbnail of its own. */
  fallbackArt: string | null;
  color: string | null;
  /** Watch Progress, 0–1. */
  progress?: number;
  onPlay?: () => void;
};

export function EpisodeCard({ episode, fallbackArt, color, progress, onPlay }: EpisodeCardProps) {
  const upcoming = episode.airingAt !== null;
  const title = episode.title ?? `Episode ${episode.number}`;

  return (
    <button type="button" className={styles.card} onClick={onPlay} disabled={upcoming} aria-label={`Play episode ${episode.number}: ${title}`}>
      <div className={styles.thumb}>
        <Artwork src={episode.thumbnailUrl ?? fallbackArt} alt="" color={color} ratio="wide" className={episode.thumbnailUrl ? undefined : styles.dimmed} />
        {!upcoming && (
          <span className={styles.play} aria-hidden="true">
            <Play />
          </span>
        )}
        {upcoming && <span className={styles.badge}>Airs {airingLabel(episode.airingAt as number)}</span>}
        {progress !== undefined && progress > 0 && (
          <span className={styles.progress} aria-hidden="true">
            <span style={{ width: `${Math.min(progress, 1) * 100}%` }} />
          </span>
        )}
      </div>
      <div className={styles.text}>
        <span className={styles.number}>Episode {episode.number}</span>
        {episode.title && <span className={styles.title}>{episode.title}</span>}
      </div>
    </button>
  );
}
