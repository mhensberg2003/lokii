import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { Info, Play, WifiOff } from "lucide-react";
import { catalog, type ShowDetailsLite } from "../lib/catalog";
import { scoreLabel, showMeta } from "../lib/format";
import { Button } from "../components/ui/Button";
import { ShowRowView, SkeletonRows } from "../components/ShowRows";
import { ContinueRow } from "../components/ContinueRow";
import { PageMessage, errorText } from "../components/PageMessage";
import styles from "./Home.module.css";

export function Home() {
  const home = useQuery({ queryKey: ["catalog", "home"], queryFn: catalog.home });

  if (home.isError) {
    return (
      <PageMessage icon={WifiOff} title="Home could not load" onRetry={() => home.refetch()}>
        {errorText(home.error)}
      </PageMessage>
    );
  }

  return (
    <div className={styles.page}>
      {home.data ? <Hero show={home.data.hero} /> : <div className={styles.heroSkeleton} />}
      <div className={styles.rows}>
        <ContinueRow />
        {home.data ? home.data.rows.map((row) => <ShowRowView key={row.id} row={row} />) : <SkeletonRows />}
      </div>
    </div>
  );
}

function Hero({ show }: { show: ShowDetailsLite }) {
  const navigate = useNavigate();
  const score = scoreLabel(show.averageScore);
  const art = show.bannerUrl ?? show.coverUrl;

  return (
    <section className={styles.hero} aria-label={`Featured: ${show.title}`}>
      <img className={styles.heroArt} src={art} alt="" draggable={false} />
      <div className={styles.heroShade} />
      <div className={styles.heroContent}>
        <p className={styles.heroMeta}>
          {showMeta(show)}
          {score && <span className={styles.score}>★ {score}</span>}
        </p>
        <h1 className={styles.heroTitle} data-long={show.title.length > 28}>{show.title}</h1>
        <p className={styles.heroDescription}>{show.description}</p>
        <div className={styles.heroActions}>
          <Button variant="primary" size="lg" icon={<Play fill="currentColor" />} onClick={() => navigate(`/show/${show.id}`)}>
            Play
          </Button>
          <Button variant="secondary" size="lg" icon={<Info />} onClick={() => navigate(`/show/${show.id}`)}>
            More info
          </Button>
        </div>
      </div>
    </section>
  );
}
