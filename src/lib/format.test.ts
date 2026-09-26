import { describe, expect, it } from "vitest";
import { airingLabel, durationLabel, formatLabel, scoreLabel, seasonLabel, showMeta } from "./format";

describe("formatLabel", () => {
  it("maps known AniList formats", () => {
    expect(formatLabel("TV_SHORT")).toBe("TV Short");
    expect(formatLabel("MOVIE")).toBe("Movie");
  });
  it("passes unknown formats through and handles null", () => {
    expect(formatLabel("NEW_FORMAT")).toBe("NEW_FORMAT");
    expect(formatLabel(null)).toBeNull();
  });
});

describe("seasonLabel", () => {
  it("formats season and year", () => {
    expect(seasonLabel("FALL", 2026)).toBe("Fall 2026");
  });
  it("falls back to the year, or null", () => {
    expect(seasonLabel(null, 2026)).toBe("2026");
    expect(seasonLabel("FALL", null)).toBeNull();
  });
});

describe("showMeta", () => {
  it("joins the parts that exist", () => {
    expect(showMeta({ format: "TV", season: "WINTER", seasonYear: 2026, episodes: 12 })).toBe("TV · Winter 2026 · 12 eps");
    expect(showMeta({ format: "MOVIE", season: null, seasonYear: 2024, episodes: 1 })).toBe("Movie · 2024 · 1 ep");
    expect(showMeta({ format: null, season: null, seasonYear: null, episodes: null })).toBe("");
  });
});

describe("scoreLabel", () => {
  it("converts 0–100 to one decimal", () => {
    expect(scoreLabel(87)).toBe("8.7");
  });
  it("hides missing scores", () => {
    expect(scoreLabel(null)).toBeNull();
    expect(scoreLabel(0)).toBeNull();
  });
});

describe("durationLabel", () => {
  it("formats minutes and hours", () => {
    expect(durationLabel(24)).toBe("24m");
    expect(durationLabel(120)).toBe("2h");
    expect(durationLabel(95)).toBe("1h 35m");
    expect(durationLabel(null)).toBeNull();
  });
});

describe("airingLabel", () => {
  const now = 1_000_000;
  it("uses minutes, hours or days", () => {
    expect(airingLabel(now + 12 * 60, now)).toBe("in 12 min");
    expect(airingLabel(now + 3600, now)).toBe("in 1 hour");
    expect(airingLabel(now + 5 * 3600, now)).toBe("in 5 hours");
    expect(airingLabel(now + 3 * 86400, now)).toBe("in 3 days");
  });
  it("never goes negative", () => {
    expect(airingLabel(now - 100, now)).toBe("in 0 min");
  });
});
