import { describe, expect, it } from "vitest";
import type { ShowDetails } from "./catalog";
import {
  activeSegment,
  formatTime,
  languageName,
  nextEpisodeDue,
  nextEpisodeOf,
  shortcutFor,
  trackLabel,
  type SkipSegment,
} from "./player";

const SEGMENTS: SkipSegment[] = [
  { kind: "intro", start: 90, end: 180 },
  { kind: "outro", start: 1340, end: 1430 },
];

function show(overrides: Partial<ShowDetails>): ShowDetails {
  return {
    id: 1,
    episodeList: [],
    franchise: [],
    ...overrides,
  } as ShowDetails;
}

const episode = (number: number, airingAt: number | null = null) => ({
  number,
  title: `Episode title ${number}`,
  thumbnailUrl: null,
  airingAt,
});

const entry = (id: number, format: string, status: string | null = "FINISHED") => ({
  id,
  title: `Show ${id}`,
  format,
  status,
  season: null,
  seasonYear: null,
  episodes: 12,
  label: null,
});

describe("formatTime", () => {
  it("shows minutes, and hours from one hour", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(247.9)).toBe("4:07");
    expect(formatTime(3729)).toBe("1:02:09");
    expect(formatTime(Number.NaN)).toBe("0:00");
  });
});

describe("activeSegment", () => {
  it("finds the segment that contains the time", () => {
    expect(activeSegment(SEGMENTS, 89)).toBeNull();
    expect(activeSegment(SEGMENTS, 90)?.kind).toBe("intro");
    expect(activeSegment(SEGMENTS, 180)).toBeNull();
    expect(activeSegment(SEGMENTS, 1400)?.kind).toBe("outro");
  });
});

describe("nextEpisodeOf", () => {
  it("plays the next aired Episode of the Show", () => {
    const next = nextEpisodeOf(show({ episodeList: [episode(1), episode(2)] }), 1);
    expect(next).toEqual({ showId: 1, episode: 2, title: "Episode title 2", showTitle: null });
  });

  it("has no next Episode before it airs", () => {
    expect(nextEpisodeOf(show({ episodeList: [episode(1), episode(2, 1_900_000_000)] }), 1)).toBeNull();
  });

  it("continues with the next TV Show in the Franchise, skipping movies", () => {
    const details = show({
      episodeList: [episode(1), episode(2)],
      franchise: [entry(1, "TV"), entry(5, "MOVIE"), entry(9, "TV")],
    });
    expect(nextEpisodeOf(details, 2)).toEqual({ showId: 9, episode: 1, title: null, showTitle: "Show 9" });
  });

  it("stops at the end of the Franchise or before a sequel airs", () => {
    expect(nextEpisodeOf(show({ episodeList: [episode(1)], franchise: [entry(1, "TV")] }), 1)).toBeNull();
    const upcoming = show({ episodeList: [episode(1)], franchise: [entry(1, "TV"), entry(2, "TV", "NOT_YET_RELEASED")] });
    expect(nextEpisodeOf(upcoming, 1)).toBeNull();
  });

  it("does not continue into a sequel while the Show still airs", () => {
    const airing = show({ status: "RELEASING", episodeList: [episode(1)], franchise: [entry(1, "TV"), entry(2, "TV")] });
    expect(nextEpisodeOf(airing, 1)).toBeNull();
  });
});

describe("nextEpisodeDue", () => {
  it("starts at the outro when AniSkip knows it", () => {
    expect(nextEpisodeDue(1339, 1440, SEGMENTS)).toBe(false);
    expect(nextEpisodeDue(1340, 1440, SEGMENTS)).toBe(true);
  });

  it("starts in the last minute otherwise", () => {
    expect(nextEpisodeDue(1379, 1440, [])).toBe(false);
    expect(nextEpisodeDue(1380, 1440, [])).toBe(true);
    expect(nextEpisodeDue(0, 0, [])).toBe(false);
  });
});

describe("shortcutFor", () => {
  it("maps the player keys", () => {
    expect(shortcutFor(" ")).toEqual({ kind: "togglePause" });
    expect(shortcutFor("ArrowLeft")).toEqual({ kind: "seekBy", seconds: -10 });
    expect(shortcutFor("ArrowUp")).toEqual({ kind: "volumeBy", amount: 5 });
    expect(shortcutFor("s")).toEqual({ kind: "cycle", property: "sid" });
    expect(shortcutFor("a")).toEqual({ kind: "cycle", property: "aid" });
    expect(shortcutFor("x")).toBeNull();
  });
});

describe("track labels", () => {
  it("names languages from 2- and 3-letter codes", () => {
    expect(languageName("jpn")).toBe("Japanese");
    expect(languageName("en")).toBe("English");
    expect(languageName("und")).toBeNull();
    expect(languageName(null)).toBeNull();
  });

  it("uses the language first and the title as detail", () => {
    const track = { id: 2, kind: "sub" as const, title: "Full Subtitles [SubsPlease]", lang: "eng", codec: "ass", selected: true };
    expect(trackLabel(track, 1)).toEqual({ name: "English", detail: "Full Subtitles [SubsPlease]" });
    expect(trackLabel({ ...track, lang: null, title: null }, 3)).toEqual({ name: "Track 3", detail: null });
  });
});
