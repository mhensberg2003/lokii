import { useNavigate } from "react-router";
import { ArrowDownToLine, Play, Trash2 } from "lucide-react";
import {
  detailLabel,
  downloadFraction,
  progressLabel,
  sourceLabel,
  speedLabel,
  statusLabel,
  type StreamView,
} from "../lib/streams";
import { useRemoveStream, useStreams } from "../lib/useStreams";
import { Button } from "../components/ui/Button";
import { PageMessage } from "../components/PageMessage";
import { StreamIcon } from "../components/StreamIcon";
import styles from "./Downloads.module.css";

/** Every Stream of this session with its torrent details: speed, peers and size. */
export function Downloads() {
  const { data: list = [], isPending } = useStreams();

  if (isPending) return null;
  if (list.length === 0) {
    return (
      <PageMessage icon={ArrowDownToLine} title="No downloads">
        Episodes that stream through TorBox or a local torrent will appear here.
      </PageMessage>
    );
  }
  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>Downloads</h1>
        <p className={styles.note}>
          Lokii deletes local data when you quit, and after you watch an Episode. TorBox keeps a Batch until you watch
          its last Episode.
        </p>
      </header>
      <ul className={styles.list}>
        {list.map((stream) => (
          <li key={stream.id}>
            <DownloadRow stream={stream} />
          </li>
        ))}
      </ul>
    </div>
  );
}

function DownloadRow({ stream }: { stream: StreamView }) {
  const navigate = useNavigate();
  const remove = useRemoveStream();
  const fraction = downloadFraction(stream);
  const stats = [
    sourceLabel(stream.source),
    progressLabel(stream),
    speedLabel(stream.speed),
    stream.peers > 0 ? `${stream.peers} ${stream.source === "torbox" ? "seeds" : "peers"}` : null,
  ].filter(Boolean);

  return (
    <article className={styles.row} data-failed={stream.phase.kind === "failed"}>
      <StreamIcon stream={stream} />
      <div className={styles.main}>
        <div className={styles.top}>
          <h2 className={styles.name}>
            {stream.showTitle || "Finding the Show"} · Episode {stream.episode}
          </h2>
          <span className={styles.status}>{statusLabel(stream)}</span>
        </div>
        <p className={styles.detail}>{detailLabel(stream)}</p>
        {fraction !== null && stream.phase.kind !== "failed" && (
          <span className={styles.bar} aria-hidden="true">
            <span style={{ width: `${fraction * 100}%` }} />
          </span>
        )}
        <p className={styles.stats}>{stats.join(" · ")}</p>
        {(stream.fileName ?? stream.releaseTitle) && (
          <p className={styles.file} title={stream.releaseTitle ?? undefined}>
            {stream.fileName ?? stream.releaseTitle}
          </p>
        )}
      </div>
      <div className={styles.actions}>
        <Button
          size="sm"
          icon={<Play />}
          disabled={stream.phase.kind === "failed"}
          onClick={() => navigate(`/watch/${stream.showId}/${stream.episode}`)}
        >
          Play
        </Button>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon={<Trash2 />}
          aria-label={`Remove Episode ${stream.episode}`}
          disabled={remove.isPending}
          onClick={() => remove.mutate(stream.id)}
        />
      </div>
    </article>
  );
}
