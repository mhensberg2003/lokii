import { Check } from "lucide-react";
import { player, trackLabel, type Track, type TrackKind } from "../lib/player";
import styles from "./Player.module.css";

type OptionProps = { name: string; detail?: string | null; selected: boolean; onSelect: () => void };

function TrackOption({ name, detail, selected, onSelect }: OptionProps) {
  return (
    <li>
      <button type="button" className={styles.trackOption} aria-pressed={selected} onClick={onSelect}>
        <span className={styles.trackCheck}>{selected && <Check />}</span>
        <span className={styles.trackText}>
          <span>{name}</span>
          {detail && <span className={styles.trackDetail}>{detail}</span>}
        </span>
      </button>
    </li>
  );
}

type ColumnProps = { title: string; kind: TrackKind; tracks: Track[]; canTurnOff: boolean };

function TrackColumn({ title, kind, tracks, canTurnOff }: ColumnProps) {
  const select = (id: number | null) => player.setTrack(kind, id).catch(() => {});
  return (
    <section className={styles.trackColumn} aria-label={title}>
      <h2 className={styles.trackHeading}>{title}</h2>
      <ul className={styles.trackList}>
        {canTurnOff && <TrackOption name="Off" selected={!tracks.some((t) => t.selected)} onSelect={() => select(null)} />}
        {tracks.map((track, index) => {
          const { name, detail } = trackLabel(track, index + 1);
          return <TrackOption key={track.id} name={name} detail={detail} selected={track.selected} onSelect={() => select(track.id)} />;
        })}
        {tracks.length === 0 && !canTurnOff && <li className={styles.trackEmpty}>No audio tracks</li>}
      </ul>
    </section>
  );
}

/** One panel with subtitles and audio side by side (issue #19). */
export function TrackPanel({ tracks }: { tracks: Track[] }) {
  return (
    <div className={styles.trackPanel} role="dialog" aria-label="Subtitles and audio">
      <TrackColumn title="Subtitles" kind="sub" tracks={tracks.filter((t) => t.kind === "sub")} canTurnOff />
      <TrackColumn title="Audio" kind="audio" tracks={tracks.filter((t) => t.kind === "audio")} canTurnOff={false} />
    </div>
  );
}
