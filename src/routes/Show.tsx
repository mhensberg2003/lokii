import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { Play, SearchX, WifiOff } from "lucide-react";
import { catalog, type ShowDetails } from "../lib/catalog";
import { airingLabel, durationLabel, formatLabel, scoreLabel, seasonLabel } from "../lib/format";
import { episodeRanges, franchiseSeasons } from "../lib/show";
import { Button } from "../components/ui/Button";
import { Tabs } from "../components/ui/Tabs";
import { EpisodeCard } from "../components/ui/EpisodeCard";
import { PosterCard } from "../components/ui/PosterCard";
import { PageMessage, errorText } from "../components/PageMessage";
import styles from "./Show.module.css";

type ShowTab = "episodes" | "related" | "details";

export function Show() {
  const id = Number(useParams().id);
  const show = useQuery({
    queryKey: ["catalog", "show", id],
    queryFn: () => catalog.show(id),
    enabled: Number.isInteger(id) && id > 0,
  });

  if (!Number.isInteger(id) || id <= 0) {
    return <PageMessage icon={SearchX} title="This Show does not exist" />;
  }
  if (show.isError) {
    return (
      <PageMessage icon={WifiOff} title="This Show could not load" onRetry={() => show.refetch()}>
        {errorText(show.error)}
      </PageMessage>
    );
  }
  if (!show.data) return <div className={styles.loading} />;
  return <ShowView key={show.data.id} show={show.data} />;
}

function ShowView({ show }: { show: ShowDetails }) {
  const navigate = useNavigate();
  const [tab, setTab] = useState<ShowTab>("episodes");
  const art = show.bannerUrl ?? show.coverUrl;
  const firstEpisode = show.episodeList.find((e) => e.airingAt === null);
  const seasons = franchiseSeasons(show.franchise);

  const tabs = useMemo(() => {
    const list: { id: ShowTab; label: string }[] = [{ id: "episodes", label: "Episodes" }];
    if (show.related.length > 0) list.push({ id: "related", label: "Related" });
    list.push({ id: "details", label: "Details" });
    return list;
  }, [show.related.length]);

  // The spike player (M0) stands in until Streams exist (M3).
  const play = () => navigate("/player");

  return (
    <div className={styles.page}>
      <section className={styles.hero}>
        <img className={styles.heroArt} src={art} alt="" draggable={false} data-cover={!show.bannerUrl} />
        <div className={styles.heroShade} />
        <div className={styles.heroContent}>
          <h1 className={styles.title}>{show.title}</h1>
          <MetaLine show={show} />
          <p className={styles.description}>{show.description}</p>
          <div className={styles.actions}>
            <Button variant="primary" size="lg" icon={<Play fill="currentColor" />} onClick={play} disabled={!firstEpisode}>
              {firstEpisode ? `Play episode ${firstEpisode.number}` : "Not aired yet"}
            </Button>
          </div>
        </div>
      </section>

      <div className={styles.body}>
        {seasons.length > 1 && (
          <div className={styles.seasons} role="group" aria-label="Seasons">
            {seasons.map((season) => (
              <button
                key={season.id}
                type="button"
                className={styles.season}
                aria-pressed={season.id === show.id}
                onClick={() => season.id !== show.id && navigate(`/show/${season.id}`, { replace: true })}
              >
                <span>{season.label}</span>
                {season.detail && <span className={styles.seasonDetail}>{season.detail}</span>}
              </button>
            ))}
          </div>
        )}

        <Tabs label="Show sections" tabs={tabs} value={tab} onChange={setTab} />

        {tab === "episodes" && <EpisodeGrid show={show} onPlay={play} />}
        {tab === "related" && (
          <div className={styles.posterGrid}>
            {show.related.map((related) => (
              <PosterCard key={related.id} show={related} />
            ))}
          </div>
        )}
        {tab === "details" && <Details show={show} />}
      </div>
    </div>
  );
}

function MetaLine({ show }: { show: ShowDetails }) {
  const score = scoreLabel(show.averageScore);
  const parts = [
    formatLabel(show.format),
    seasonLabel(show.season, show.seasonYear),
    show.episodes ? `${show.episodes} episodes` : null,
    durationLabel(show.duration),
  ].filter(Boolean);

  return (
    <div className={styles.meta}>
      {score && <span className={styles.score}>★ {score}</span>}
      <span>{parts.join(" · ")}</span>
      {show.nextAiring && (
        <span className={styles.airing}>
          Episode {show.nextAiring.episode} {airingLabel(show.nextAiring.airingAt)}
        </span>
      )}
      {show.genres.length > 0 && <span className={styles.genres}>{show.genres.slice(0, 4).join(", ")}</span>}
    </div>
  );
}

function EpisodeGrid({ show, onPlay }: { show: ShowDetails; onPlay: () => void }) {
  const ranges = episodeRanges(show.episodeList.length);
  const [rangeId, setRangeId] = useState(ranges[0]?.id ?? "all");
  const range = ranges.find((r) => r.id === rangeId);
  const episodes = range ? show.episodeList.filter((e) => e.number >= range.start && e.number <= range.end) : show.episodeList;

  if (show.episodeList.length === 0) {
    return <p className={styles.empty}>AniList has no episode count for this Show yet.</p>;
  }

  return (
    <div className={styles.episodes}>
      {ranges.length > 0 && (
        <div className={styles.ranges} role="group" aria-label="Episode range">
          {ranges.map((r) => (
            <button key={r.id} type="button" className={styles.range} aria-pressed={r.id === rangeId} onClick={() => setRangeId(r.id)}>
              {r.label}
            </button>
          ))}
        </div>
      )}
      <div className={styles.episodeGrid}>
        {episodes.map((episode) => (
          <EpisodeCard
            key={episode.number}
            episode={episode}
            fallbackArt={show.bannerUrl ?? show.coverUrl}
            color={show.color}
            onPlay={onPlay}
          />
        ))}
      </div>
    </div>
  );
}

function Details({ show }: { show: ShowDetails }) {
  const rows: [string, string | null][] = [
    ["Romaji", show.titleRomaji],
    ["Native", show.titleNative],
    ["Format", formatLabel(show.format)],
    ["Episodes", show.episodes ? String(show.episodes) : null],
    ["Episode length", durationLabel(show.duration)],
    ["Status", show.status ? show.status.charAt(0) + show.status.slice(1).toLowerCase().replace(/_/g, " ") : null],
    ["Season", seasonLabel(show.season, show.seasonYear)],
    ["Studios", show.studios.join(", ") || null],
    ["Genres", show.genres.join(", ") || null],
    ["Score", scoreLabel(show.averageScore)],
  ];

  return (
    <dl className={styles.details}>
      {rows
        .filter((row): row is [string, string] => row[1] !== null)
        .map(([label, value]) => (
          <div key={label} className={styles.detailRow}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
    </dl>
  );
}
