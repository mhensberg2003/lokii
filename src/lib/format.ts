import type { Season, ShowCard } from "./catalog";

const FORMAT_LABELS: Record<string, string> = {
  TV: "TV",
  TV_SHORT: "TV Short",
  MOVIE: "Movie",
  SPECIAL: "Special",
  OVA: "OVA",
  ONA: "ONA",
  MUSIC: "Music",
};

export function formatLabel(format: string | null): string | null {
  if (!format) return null;
  return FORMAT_LABELS[format] ?? format;
}

export function seasonLabel(season: Season | null, year: number | null): string | null {
  if (!year) return null;
  if (!season) return String(year);
  return `${season.charAt(0)}${season.slice(1).toLowerCase()} ${year}`;
}

/** "TV · Fall 2026 · 12 eps" — only the parts that exist. */
export function showMeta(show: Pick<ShowCard, "format" | "season" | "seasonYear" | "episodes">): string {
  const parts = [
    formatLabel(show.format),
    seasonLabel(show.season, show.seasonYear),
    show.episodes ? `${show.episodes} ${show.episodes === 1 ? "ep" : "eps"}` : null,
  ];
  return parts.filter(Boolean).join(" · ");
}

/** AniList scores are 0–100; show them as 0–10 with one decimal. */
export function scoreLabel(score: number | null): string | null {
  if (score === null || score <= 0) return null;
  return (score / 10).toFixed(1);
}

export function durationLabel(minutes: number | null): string | null {
  if (!minutes || minutes <= 0) return null;
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `${h}h ${m}m` : `${h}h`;
}

/** Relative airing time: "in 3 days", "in 5 hours", "in 12 min". */
export function airingLabel(airingAt: number, now: number = Date.now() / 1000): string {
  const seconds = Math.max(0, airingAt - now);
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `in ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `in ${hours} ${hours === 1 ? "hour" : "hours"}`;
  const days = Math.round(hours / 24);
  return `in ${days} days`;
}
