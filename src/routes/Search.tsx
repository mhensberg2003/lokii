import { useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import { Search as SearchIcon, SearchX, WifiOff } from "lucide-react";
import { catalog } from "../lib/catalog";
import { PosterCard, PosterCardSkeleton } from "../components/ui/PosterCard";
import { PageMessage, errorText } from "../components/PageMessage";
import styles from "./Search.module.css";

const MIN_QUERY_LENGTH = 2;

export function Search() {
  const [params] = useSearchParams();
  const query = (params.get("q") ?? "").trim();
  const enabled = query.length >= MIN_QUERY_LENGTH;

  const results = useQuery({
    queryKey: ["catalog", "search", query.toLowerCase()],
    queryFn: () => catalog.search(query),
    enabled,
    placeholderData: (previous) => previous,
  });

  if (!enabled) {
    return (
      <PageMessage icon={SearchIcon} title="Search anime">
        Type at least {MIN_QUERY_LENGTH} characters.
      </PageMessage>
    );
  }
  if (results.isError) {
    return (
      <PageMessage icon={WifiOff} title="Search failed" onRetry={() => results.refetch()}>
        {errorText(results.error)}
      </PageMessage>
    );
  }
  if (results.data && results.data.length === 0) {
    return <PageMessage icon={SearchX} title={`No Shows match “${query}”`}>Try the Japanese or English title.</PageMessage>;
  }

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>
        Results for <span className={styles.query}>{query}</span>
      </h1>
      <div className={styles.grid} data-stale={results.isPlaceholderData}>
        {results.data
          ? results.data.map((show) => <PosterCard key={show.id} show={show} />)
          : Array.from({ length: 12 }, (_, i) => <PosterCardSkeleton key={i} />)}
      </div>
    </div>
  );
}
