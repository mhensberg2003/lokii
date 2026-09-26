import { describe, expect, it } from "vitest";
import { batchLabel, chosenRelease, resolutionLabel, resolutionsIn, sizeLabel, type Release } from "./releases";

const release = (infoHash: string, resolution: number | null): Release => ({
  infoHash,
  title: `[Group] Show - 01 (${resolution}p)`,
  group: "Group",
  resolution,
  sizeBytes: 1,
  seeders: 1,
  leechers: 0,
  fileCount: 1,
  coverage: { kind: "episode", episode: 1 },
  isBest: false,
  publishedAt: 0,
  magnet: "",
  link: "",
});

describe("release labels", () => {
  it("formats resolutions", () => {
    expect(resolutionLabel(1080)).toBe("1080p");
    expect(resolutionLabel(2160)).toBe("4K");
    expect(resolutionLabel(null)).toBeNull();
  });

  it("formats sizes in MB and GB", () => {
    expect(sizeLabel(0)).toBeNull();
    expect(sizeLabel(820 * 1024 ** 2)).toBe("820 MB");
    expect(sizeLabel(45.34 * 1024 ** 3)).toBe("45.3 GB");
    expect(sizeLabel(123.4 * 1024 ** 3)).toBe("123 GB");
  });

  it("labels Batches only", () => {
    expect(batchLabel({ kind: "episode", episode: 3 })).toBeNull();
    expect(batchLabel({ kind: "range", first: 1, last: 6 })).toBe("Batch 1–6");
    expect(batchLabel({ kind: "show" })).toBe("Batch");
  });
});

describe("release lists", () => {
  it("finds the Chosen Release", () => {
    const list = [release("a", 720), release("b", 1080)];
    expect(chosenRelease({ showId: 1, episode: 1, releases: list, chosen: "b", pickedByUser: false })?.infoHash).toBe("b");
    expect(chosenRelease({ showId: 1, episode: 1, releases: list, chosen: null, pickedByUser: false })).toBeNull();
  });

  it("lists distinct resolutions, highest first", () => {
    expect(resolutionsIn([release("a", 720), release("b", 1080), release("c", 720), release("d", null)])).toEqual([1080, 720]);
  });
});
