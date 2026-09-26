import { describe, expect, it } from "vitest";
import { episodeRanges, franchiseSeasons } from "./show";
import type { FranchiseEntry } from "./catalog";

const entry = (id: number, seasonYear: number | null, title = `Show ${id}`): FranchiseEntry => ({
  id,
  title,
  format: "TV",
  status: "FINISHED",
  season: null,
  seasonYear,
  episodes: 12,
});

describe("franchiseSeasons", () => {
  it("uses each title minus the Franchise name", () => {
    const aot = [
      entry(1, 2013, "Attack on Titan"),
      entry(2, 2017, "Attack on Titan Season 2"),
      entry(3, 2019, "Attack on Titan Season 3 Part 2"),
      entry(4, 2021, "Attack on Titan: Final Season"),
    ];
    expect(franchiseSeasons(aot).map((s) => s.label)).toEqual(["Season 1", "Season 2", "Season 3 Part 2", "Final Season"]);
  });

  it("numbers seasons when titles do not share the Franchise name", () => {
    expect(franchiseSeasons([entry(10, 2019), entry(20, 2021), entry(30, null)])).toEqual([
      { id: 10, label: "Season 1", detail: "2019" },
      { id: 20, label: "Season 2", detail: "2021" },
      { id: 30, label: "Season 3", detail: null },
    ]);
  });

  it("returns an empty list for an empty Franchise", () => {
    expect(franchiseSeasons([])).toEqual([]);
  });
});

describe("episodeRanges", () => {
  it("returns no ranges when one page is enough", () => {
    expect(episodeRanges(12)).toEqual([]);
    expect(episodeRanges(50)).toEqual([]);
  });

  it("splits into pages with a short last page", () => {
    const ranges = episodeRanges(120);
    expect(ranges.map((r) => r.label)).toEqual(["1–50", "51–100", "101–120"]);
    expect(ranges[2]).toMatchObject({ start: 101, end: 120 });
  });

  it("supports a custom page size", () => {
    expect(episodeRanges(25, 10).map((r) => r.id)).toEqual(["1-10", "11-20", "21-25"]);
  });
});
