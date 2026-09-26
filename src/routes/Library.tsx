import { useQuery } from "@tanstack/react-query";
import { LibraryBig } from "lucide-react";
import { library, libraryKeys } from "../lib/library";
import { ContinueRow } from "../components/ContinueRow";
import { PageMessage } from "../components/PageMessage";
import { PosterCard } from "../components/ui/PosterCard";
import styles from "./Library.module.css";

/** The Library page: Continue watching and the Watchlist. */
export function Library() {
  const continueWatching = useQuery({ queryKey: libraryKeys.continueWatching, queryFn: library.continueWatching });
  const watchlist = useQuery({ queryKey: libraryKeys.watchlist, queryFn: library.watchlist });

  if (watchlist.isPending) return <div className={styles.page} />;
  // Continue watching can take longer; the Watchlist does not wait for it.
  const empty = !continueWatching.isPending && (continueWatching.data?.length ?? 0) === 0 && watchlist.data?.length === 0;
  if (empty) {
    return (
      <PageMessage icon={LibraryBig} title="Your Library is empty">
        Add a Show to your Watchlist from its page, or play an Episode. It will show here.
      </PageMessage>
    );
  }

  return (
    <div className={styles.page}>
      <h1 className={styles.heading}>Library</h1>
      <ContinueRow />
      <section className={styles.section} aria-label="Watchlist">
        <h2 className={styles.title}>Watchlist</h2>
        {watchlist.data && watchlist.data.length > 0 ? (
          <div className={styles.grid}>
            {watchlist.data.map((show) => (
              <PosterCard key={show.id} show={show} />
            ))}
          </div>
        ) : (
          <p className={styles.empty}>Shows you add to your Watchlist show here.</p>
        )}
      </section>
    </div>
  );
}
