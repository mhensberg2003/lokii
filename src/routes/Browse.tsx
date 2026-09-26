import { useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import { WifiOff } from "lucide-react";
import { BROWSE_GENRES, catalog, type BrowseGenre } from "../lib/catalog";
import { Tabs } from "../components/ui/Tabs";
import { ShowRowView, SkeletonRows } from "../components/ShowRows";
import { PageMessage, errorText } from "../components/PageMessage";
import styles from "./Browse.module.css";

const GENRE_TABS = BROWSE_GENRES.map((genre) => ({ id: genre, label: genre }));

function isGenre(value: string | null): value is BrowseGenre {
  return BROWSE_GENRES.includes(value as BrowseGenre);
}

export function Browse() {
  const [params, setParams] = useSearchParams();
  const requested = params.get("genre");
  const genre: BrowseGenre = isGenre(requested) ? requested : BROWSE_GENRES[0];

  const feed = useQuery({
    queryKey: ["catalog", "browse", genre],
    queryFn: () => catalog.browse(genre),
    placeholderData: (previous) => previous,
  });

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>Browse</h1>
        <Tabs
          label="Genres"
          tabs={GENRE_TABS}
          value={genre}
          onChange={(next) => setParams({ genre: next }, { replace: true })}
          className={styles.tabs}
        />
      </header>

      {feed.isError ? (
        <PageMessage icon={WifiOff} title={`${genre} could not load`} onRetry={() => feed.refetch()}>
          {errorText(feed.error)}
        </PageMessage>
      ) : (
        <div className={styles.rows} data-stale={feed.isPlaceholderData}>
          {feed.data ? feed.data.rows.map((row) => <ShowRowView key={`${genre}-${row.id}`} row={row} />) : <SkeletonRows count={4} />}
        </div>
      )}
    </div>
  );
}
