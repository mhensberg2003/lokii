import { batchLabel, chosenRelease, resolutionLabel, sizeLabel } from "../lib/releases";
import { useEpisodeReleases } from "../lib/useReleases";
import { errorText } from "./PageMessage";
import styles from "./ReleaseLine.module.css";

type ReleaseLineProps = {
  showId: number;
  episode: number;
  onChange: () => void;
};

/** The Chosen Release for the Episode the Play button plays, with a "Change" link. */
export function ReleaseLine({ showId, episode, onChange }: ReleaseLineProps) {
  const query = useEpisodeReleases(showId, episode);

  if (query.isPending) {
    return (
      <p className={styles.line} aria-live="polite">
        <span className={styles.pulse} aria-hidden="true" />
        Finding releases…
      </p>
    );
  }

  if (query.isError) {
    return (
      <p className={styles.line} title={errorText(query.error)}>
        <span>Releases could not load.</span>
        <button type="button" className={styles.link} onClick={() => query.refetch()}>
          Try again
        </button>
      </p>
    );
  }

  const chosen = chosenRelease(query.data);
  if (!chosen) {
    return (
      <p className={styles.line}>
        <span>No releases found for episode {episode}.</span>
        {query.data.releases.length > 0 && (
          <button type="button" className={styles.link} onClick={onChange}>
            See all
          </button>
        )}
      </p>
    );
  }

  const parts = [chosen.group, resolutionLabel(chosen.resolution), batchLabel(chosen.coverage), sizeLabel(chosen.sizeBytes)];
  return (
    <p className={styles.line}>
      <span className={styles.summary} title={chosen.title}>
        {parts.filter(Boolean).join(" · ")}
      </span>
      {chosen.isBest && <span className={styles.best}>Best release</span>}
      {query.data.pickedByUser && <span className={styles.picked}>Your pick</span>}
      <button type="button" className={styles.link} onClick={onChange}>
        Change release
      </button>
    </p>
  );
}
