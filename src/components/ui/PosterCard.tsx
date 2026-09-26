import { Link } from "react-router";
import type { ShowCard } from "../../lib/catalog";
import { showMeta } from "../../lib/format";
import { Artwork } from "./Artwork";
import styles from "./PosterCard.module.css";

export function PosterCard({ show }: { show: ShowCard }) {
  return (
    <Link to={`/show/${show.id}`} className={styles.card} draggable={false}>
      <Artwork src={show.coverUrl} alt="" color={show.color} ratio="poster" className={styles.art} />
      <div className={styles.text}>
        <span className={styles.title}>{show.title}</span>
        <span className={styles.meta}>{showMeta(show)}</span>
      </div>
    </Link>
  );
}

export function PosterCardSkeleton() {
  return (
    <div className={styles.card} aria-hidden="true">
      <div className={`${styles.art} ${styles.skeletonArt}`} />
      <div className={styles.text}>
        <span className={styles.skeletonLine} />
        <span className={`${styles.skeletonLine} ${styles.short}`} />
      </div>
    </div>
  );
}
