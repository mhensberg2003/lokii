import { useEffect, useRef, useState } from "react";
import type { UseQueryResult } from "@tanstack/react-query";
import { ArrowUp, Check, ChevronLeft, ChevronRight, X } from "lucide-react";
import type { ShowDetails } from "../lib/catalog";
import { useEpisodeReleases, usePickRelease } from "../lib/useReleases";
import {
  batchLabel,
  resolutionLabel,
  resolutionsIn,
  sizeLabel,
  type EpisodeReleases,
  type Release,
} from "../lib/releases";
import { Button } from "./ui/Button";
import { errorText } from "./PageMessage";
import styles from "./ReleaseDialog.module.css";

type ReleaseDialogProps = {
  show: ShowDetails;
  episode: number;
  onClose: () => void;
};

/** The full Release list for one Episode. Picking a Release sets the Chosen Release for the Show. */
export function ReleaseDialog({ show, episode: initialEpisode, onClose }: ReleaseDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [episode, setEpisode] = useState(initialEpisode);
  const [resolution, setResolution] = useState<number | null>(null);
  const query = useEpisodeReleases(show.id, episode);
  const pick = usePickRelease(show.id, episode);
  const aired = show.episodeList.filter((e) => e.airingAt === null).map((e) => e.number);

  useEffect(() => dialog.current?.showModal(), []);

  return (
    <dialog
      ref={dialog}
      className={styles.dialog}
      aria-labelledby="release-dialog-title"
      onClose={onClose}
      onClick={(event) => event.target === dialog.current && dialog.current?.close()}
    >
      <header className={styles.header}>
        <div className={styles.heading}>
          <h2 id="release-dialog-title" className={styles.title}>
            Change release
          </h2>
          <p className={styles.subtitle}>Your pick applies to every episode of {show.title}.</p>
        </div>
        <Button variant="ghost" iconOnly icon={<X />} aria-label="Close" onClick={() => dialog.current?.close()} />
      </header>

      <div className={styles.toolbar}>
        <EpisodeStepper episodes={aired} value={episode} onChange={setEpisode} />
        {query.data && <ResolutionFilter list={query.data.releases} value={resolution} onChange={setResolution} />}
      </div>

      <div className={styles.body}>
        <ReleaseListBody query={query} resolution={resolution} busy={pick.isPending} onPick={(hash) => pick.mutate(hash)} />
      </div>

      <footer className={styles.footer}>
        <span className={styles.note}>
          {pick.isError ? errorText(pick.error) : "Automatic: the Best Release, else the 1080p release with the most seeders."}
        </span>
        {query.data?.pickedByUser && (
          <Button size="sm" onClick={() => pick.mutate(null)} disabled={pick.isPending}>
            Use automatic
          </Button>
        )}
      </footer>
    </dialog>
  );
}

type EpisodeStepperProps = { episodes: number[]; value: number; onChange: (episode: number) => void };

function EpisodeStepper({ episodes, value, onChange }: EpisodeStepperProps) {
  const index = episodes.indexOf(value);
  const step = (delta: number) => {
    const next = episodes[index + delta];
    if (next !== undefined) onChange(next);
  };
  return (
    <div className={styles.stepper}>
      <Button variant="ghost" size="sm" iconOnly icon={<ChevronLeft />} aria-label="Previous episode" disabled={index <= 0} onClick={() => step(-1)} />
      <span className={styles.episode}>Episode {value}</span>
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        icon={<ChevronRight />}
        aria-label="Next episode"
        disabled={index < 0 || index >= episodes.length - 1}
        onClick={() => step(1)}
      />
    </div>
  );
}

type ListBodyProps = {
  query: UseQueryResult<EpisodeReleases>;
  resolution: number | null;
  busy: boolean;
  onPick: (infoHash: string) => void;
};

function ReleaseListBody({ query, resolution, busy, onPick }: ListBodyProps) {
  if (query.isPending) return <p className={styles.message}>Finding releases…</p>;
  if (query.isError) {
    return (
      <div className={styles.message}>
        <p>{errorText(query.error)}</p>
        <Button size="sm" onClick={() => query.refetch()}>
          Try again
        </Button>
      </div>
    );
  }
  const list = query.data.releases.filter((r) => resolution === null || r.resolution === resolution);
  if (list.length === 0) return <p className={styles.message}>No releases found for this episode.</p>;

  return (
    <ul className={styles.list} role="radiogroup" aria-label="Releases" aria-busy={busy}>
      {list.map((release) => (
        <li key={release.infoHash}>
          <ReleaseRow release={release} chosen={release.infoHash === query.data.chosen} disabled={busy} onPick={onPick} />
        </li>
      ))}
    </ul>
  );
}

function ReleaseRow({ release, chosen, disabled, onPick }: { release: Release; chosen: boolean; disabled: boolean; onPick: (hash: string) => void }) {
  const batch = batchLabel(release.coverage);
  return (
    <button
      type="button"
      role="radio"
      aria-checked={chosen}
      className={styles.row}
      disabled={disabled}
      onClick={() => !chosen && onPick(release.infoHash)}
    >
      <span className={styles.check} aria-hidden="true">
        {chosen && <Check />}
      </span>
      <span className={styles.main}>
        <span className={styles.rowTop}>
          <span className={styles.group}>{release.group ?? "Unknown group"}</span>
          {release.resolution && <span className={styles.chip}>{resolutionLabel(release.resolution)}</span>}
          {batch && <span className={styles.chip}>{batch}</span>}
          {release.isBest && <span className={styles.best}>Best release</span>}
        </span>
        <span className={styles.name} title={release.title}>
          {release.title}
        </span>
      </span>
      <span className={styles.stats}>
        <span>{sizeLabel(release.sizeBytes) ?? "–"}</span>
        <span className={styles.seeders} data-dead={release.seeders === 0} aria-label={`${release.seeders} seeders`}>
          <ArrowUp aria-hidden="true" />
          {release.seeders}
        </span>
      </span>
    </button>
  );
}

function ResolutionFilter({ list, value, onChange }: { list: Release[]; value: number | null; onChange: (r: number | null) => void }) {
  const options = resolutionsIn(list);
  if (options.length < 2) return null;
  return (
    <div className={styles.filter} role="group" aria-label="Resolution">
      <button type="button" className={styles.filterOption} aria-pressed={value === null} onClick={() => onChange(null)}>
        All {list.length}
      </button>
      {options.map((r) => (
        <button key={r} type="button" className={styles.filterOption} aria-pressed={value === r} onClick={() => onChange(r)}>
          {resolutionLabel(r)}
        </button>
      ))}
    </div>
  );
}
