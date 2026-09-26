import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { Play } from "lucide-react";
import { continueLabel, library, libraryKeys, progressFraction, type ContinueItem } from "../lib/library";
import { Artwork } from "./ui/Artwork";
import { Row } from "./ui/Row";
import styles from "./ContinueRow.module.css";

/** The Continue watching row. Hidden until the user played an Episode. */
export function ContinueRow() {
  const items = useQuery({ queryKey: libraryKeys.continueWatching, queryFn: library.continueWatching }).data ?? [];
  if (items.length === 0) return null;
  return (
    <Row title="Continue watching" itemWidth={280}>
      {items.map((item) => (
        <ContinueCard key={item.show.id} item={item} />
      ))}
    </Row>
  );
}

export function ContinueCard({ item }: { item: ContinueItem }) {
  const { show, episode, episodeTitle, thumbnailUrl } = item;
  const fraction = progressFraction({ ...item, watched: false });
  const label = `Episode ${episode} · ${continueLabel(item)}`;
  return (
    <Link to={`/watch/${show.id}/${episode}`} className={styles.card} draggable={false} aria-label={`${show.title}, ${label}`}>
      <div className={styles.thumb}>
        <Artwork
          src={thumbnailUrl ?? show.bannerUrl ?? show.coverUrl}
          alt=""
          color={show.color}
          ratio="wide"
          className={thumbnailUrl ? undefined : styles.dimmed}
        />
        <span className={styles.play} aria-hidden="true">
          <Play />
        </span>
        {fraction > 0 && (
          <span className={styles.progress} aria-hidden="true">
            <span style={{ width: `${fraction * 100}%` }} />
          </span>
        )}
      </div>
      <div className={styles.text}>
        <span className={styles.title}>{show.title}</span>
        <span className={styles.detail}>
          {label}
          {episodeTitle && ` · ${episodeTitle}`}
        </span>
      </div>
    </Link>
  );
}
