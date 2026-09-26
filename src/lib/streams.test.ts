import { describe, expect, it } from "vitest";
import { detailLabel, isActive, progressLabel, speedLabel, statusLabel, upsertStream, type StreamView } from "./streams";

function stream(overrides: Partial<StreamView> = {}): StreamView {
  return {
    id: "a",
    showId: 16498,
    episode: 1,
    showTitle: "Attack on Titan",
    source: "local",
    releaseTitle: null,
    releaseGroup: null,
    fileName: null,
    phase: { kind: "preparing", step: "Finding peers" },
    url: null,
    size: 0,
    downloaded: 0,
    speed: 0,
    peers: 0,
    watched: false,
    startedAt: 100,
    ...overrides,
  };
}

const GB = 1024 ** 3;

describe("statusLabel", () => {
  it("shows the phase in one word or a percentage", () => {
    expect(statusLabel(stream())).toBe("Preparing");
    expect(statusLabel(stream({ phase: { kind: "downloading", progress: 0.426 } }))).toBe("42%");
    expect(statusLabel(stream({ phase: { kind: "failed", message: "x" } }))).toBe("Failed");
  });

  it("shows the download of a playing Local Torrent until it is complete", () => {
    const playing = stream({ phase: { kind: "ready" }, size: 4 * GB, downloaded: GB });
    expect(statusLabel(playing)).toBe("25%");
    expect(statusLabel({ ...playing, downloaded: 4 * GB })).toBe("Ready");
    expect(statusLabel({ ...playing, source: "torbox" })).toBe("Ready");
  });
});

describe("detailLabel", () => {
  it("names the current step or the error", () => {
    expect(detailLabel(stream())).toBe("Finding peers…");
    expect(detailLabel(stream({ phase: { kind: "failed", message: "No peers." } }))).toBe("No peers.");
    expect(detailLabel(stream({ phase: { kind: "ready" }, source: "torbox" }))).toBe("Ready on TorBox");
  });
});

describe("labels", () => {
  it("formats speed and progress", () => {
    expect(speedLabel(0)).toBeNull();
    expect(speedLabel(5.25 * 1024 ** 2)).toBe("5.3 MB/s");
    expect(speedLabel(300 * 1024)).toBe("300 KB/s");
    expect(progressLabel(stream({ size: 2 * GB, downloaded: GB }))).toBe("1.0 GB of 2.0 GB");
    expect(progressLabel(stream({ size: 2 * GB, downloaded: 2 * GB }))).toBe("2.0 GB");
    expect(progressLabel(stream())).toBeNull();
  });
});

describe("isActive", () => {
  it("is true while the Stream prepares or a Local Torrent downloads", () => {
    expect(isActive(stream())).toBe(true);
    expect(isActive(stream({ phase: { kind: "ready" }, size: 10, downloaded: 5 }))).toBe(true);
    expect(isActive(stream({ phase: { kind: "ready" }, size: 10, downloaded: 10 }))).toBe(false);
    expect(isActive(stream({ phase: { kind: "failed", message: "x" } }))).toBe(false);
  });
});

describe("upsertStream", () => {
  it("replaces a Stream with the same ID and keeps the newest first", () => {
    const list = [stream({ id: "a", startedAt: 1 }), stream({ id: "b", startedAt: 2 })];
    const next = upsertStream(list, stream({ id: "a", startedAt: 3, episode: 2 }));
    expect(next.map((s) => [s.id, s.episode])).toEqual([
      ["a", 2],
      ["b", 1],
    ]);
  });
});
