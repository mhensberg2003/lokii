import { useState } from "react";
import { useLocation, useNavigate } from "react-router";
import { ChevronDown, ChevronUp } from "lucide-react";
import { detailLabel, statusLabel, type StreamView } from "../lib/streams";
import { useStreams } from "../lib/useStreams";
import { StreamIcon } from "../components/StreamIcon";
import styles from "./ActivityPanel.module.css";

/** The floating panel, bottom-right: one line per Stream. Hidden when there are no Streams. */
export function ActivityPanel() {
  const { data: list = [] } = useStreams();
  const [open, setOpen] = useState(true);
  const { pathname } = useLocation();
  const navigate = useNavigate();

  // The Downloads page shows the same list in full.
  if (list.length === 0 || pathname === "/downloads") return null;

  const Chevron = open ? ChevronDown : ChevronUp;
  return (
    <aside className={styles.panel} aria-label="Activity">
      <button type="button" className={styles.header} aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className={styles.heading}>Activity</span>
        {!open && <span className={styles.summary}>{summary(list)}</span>}
        <Chevron className={styles.chevron} aria-hidden="true" />
      </button>
      {open && (
        <ul className={styles.list}>
          {list.map((stream) => (
            <li key={stream.id}>
              <ActivityRow stream={stream} onOpen={() => navigate(`/watch/${stream.showId}/${stream.episode}`)} />
            </li>
          ))}
        </ul>
      )}
      {open && (
        <button type="button" className={styles.footer} onClick={() => navigate("/downloads")}>
          Open Downloads
        </button>
      )}
    </aside>
  );
}

function ActivityRow({ stream, onOpen }: { stream: StreamView; onOpen: () => void }) {
  const title = stream.showTitle || "Finding the Show";
  return (
    <button type="button" className={styles.row} onClick={onOpen} title={stream.releaseTitle ?? undefined}>
      <StreamIcon stream={stream} />
      <span className={styles.text}>
        <span className={styles.title}>{title}</span>
        <span className={styles.detail} data-failed={stream.phase.kind === "failed"}>
          Episode {stream.episode} · {detailLabel(stream)}
        </span>
      </span>
      <span className={styles.status}>{statusLabel(stream)}</span>
    </button>
  );
}

/** "2 streams · 1 ready" for the collapsed panel. */
function summary(list: StreamView[]): string {
  const ready = list.filter((s) => s.phase.kind === "ready").length;
  const streams = list.length === 1 ? "1 stream" : `${list.length} streams`;
  return ready > 0 ? `${streams} · ${ready} ready` : streams;
}
