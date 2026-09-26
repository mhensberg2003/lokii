import { AlertCircle, Check } from "lucide-react";
import { downloadFraction, type StreamView } from "../lib/streams";
import styles from "./StreamIcon.module.css";

const RADIUS = 8;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

/** The status of a Stream as one small icon: a spinner, a progress ring, a check or an alert. */
export function StreamIcon({ stream }: { stream: StreamView }) {
  const { kind } = stream.phase;
  if (kind === "failed") return <AlertCircle className={styles.failed} aria-hidden="true" />;
  if (kind === "preparing") return <span className={styles.spinner} aria-hidden="true" />;

  const fraction = downloadFraction(stream);
  if (kind === "ready" && (stream.source === "torbox" || fraction === null || fraction >= 1)) {
    return <Check className={styles.ready} aria-hidden="true" />;
  }
  return (
    <svg className={styles.ring} viewBox="0 0 20 20" aria-hidden="true">
      <circle className={styles.track} cx="10" cy="10" r={RADIUS} />
      <circle
        className={styles.value}
        cx="10"
        cy="10"
        r={RADIUS}
        strokeDasharray={CIRCUMFERENCE}
        strokeDashoffset={CIRCUMFERENCE * (1 - (fraction ?? 0))}
      />
    </svg>
  );
}
