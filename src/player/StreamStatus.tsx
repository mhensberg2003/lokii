import { detailLabel, type StreamView } from "../lib/streams";
import styles from "./Player.module.css";

type Props = { stream: StreamView | null; startError: string; onRetry: () => void; onBack: () => void };

/** What the Stream does before the video can play: preparing, downloading on TorBox, or failed. */
export function StreamStatus({ stream, startError, onRetry, onBack }: Props) {
  const failed = startError !== "" || stream?.phase.kind === "failed";
  const detail = startError || (stream ? detailLabel(stream) : "Starting…");
  const progress = stream?.phase.kind === "downloading" ? stream.phase.progress : null;
  return (
    <section className={styles.status} role="status" aria-live="polite">
      {!failed && <span className={styles.spinner} aria-hidden="true" />}
      <p className={styles.statusTitle}>{failed ? "This Episode cannot play" : "Getting the Episode ready"}</p>
      <p className={styles.statusDetail}>{detail}</p>
      {progress !== null && (
        <span className={styles.statusBar} aria-hidden="true">
          <span style={{ width: `${progress * 100}%` }} />
        </span>
      )}
      {stream?.releaseTitle && <p className={styles.statusRelease}>{stream.releaseTitle}</p>}
      {failed && (
        <div className={styles.statusActions}>
          <button type="button" className={styles.textButton} onClick={onRetry}>
            Try again
          </button>
          <button type="button" className={styles.textButton} onClick={onBack}>
            Back
          </button>
        </div>
      )}
    </section>
  );
}
