// Catalog data contract between the Rust core (src-tauri/src/catalog) and the UI.
// Terms follow CONTEXT.md. Field names are camelCase on both sides (serde rename_all).

import { invoke } from "@tauri-apps/api/core";

export type Season = "WINTER" | "SPRING" | "SUMMER" | "FALL";

export type ShowCard = {
  id: number;
  title: string;
  coverUrl: string;
  bannerUrl: string | null;
  /** AniList cover accent color, e.g. "#e4a15d". */
  color: string | null;
  format: string | null;
  episodes: number | null;
  season: Season | null;
  seasonYear: number | null;
  /** 0–100. */
  averageScore: number | null;
  genres: string[];
};

export type ShowRow = {
  id: string;
  title: string;
  shows: ShowCard[];
};

export type HomeFeed = {
  hero: ShowDetailsLite;
  rows: ShowRow[];
};

export type ShowDetailsLite = ShowCard & {
  /** Plain text, HTML removed. */
  description: string;
};

export type FranchiseEntry = {
  id: number;
  title: string;
  format: string | null;
  season: Season | null;
  seasonYear: number | null;
  episodes: number | null;
};

export type EpisodeInfo = {
  number: number;
  title: string | null;
  thumbnailUrl: string | null;
  /** Unix seconds; set only for Episodes that have not aired yet. */
  airingAt: number | null;
};

export type ShowDetails = ShowDetailsLite & {
  idMal: number | null;
  titleRomaji: string | null;
  titleNative: string | null;
  /** Other names AniList knows the Show by. */
  synonyms: string[];
  status: string | null;
  /** Minutes per Episode. */
  duration: number | null;
  studios: string[];
  nextAiring: { episode: number; airingAt: number } | null;
  /** Shows linked by sequel/prequel, oldest first. Includes this Show. */
  franchise: FranchiseEntry[];
  episodeList: EpisodeInfo[];
  /** Related Shows that are not in the Franchise (movies, side stories, spin-offs). */
  related: ShowCard[];
};

export type BrowseFeed = {
  genre: string;
  rows: ShowRow[];
};

export const BROWSE_GENRES = [
  "Action",
  "Adventure",
  "Comedy",
  "Drama",
  "Fantasy",
  "Romance",
  "Sci-Fi",
  "Slice of Life",
  "Mystery",
  "Psychological",
  "Sports",
  "Supernatural",
  "Horror",
  "Mecha",
  "Music",
  "Thriller",
] as const;

export type BrowseGenre = (typeof BROWSE_GENRES)[number];

export const catalog = {
  home: () => invoke<HomeFeed>("catalog_home"),
  browse: (genre: BrowseGenre) => invoke<BrowseFeed>("catalog_browse", { genre }),
  show: (id: number) => invoke<ShowDetails>("catalog_show", { id }),
  search: (query: string) => invoke<ShowCard[]>("catalog_search", { query }),
};
