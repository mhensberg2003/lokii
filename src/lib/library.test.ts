import { describe, expect, it } from "vitest";
import { continueLabel, playAction, progressFraction, resumeFrom, type WatchProgress } from "./library";

const saved = (position: number, watched = false): WatchProgress => ({
  showId: 1,
  episode: 3,
  position,
  duration: 1440,
  watched,
  updatedAt: 0,
});

describe("resumeFrom", () => {
  it("resumes an Episode the user left before the end", () => {
    expect(resumeFrom(saved(600))).toBe(600);
  });

  it("starts from the beginning when nothing useful is saved", () => {
    expect(resumeFrom(undefined)).toBeNull();
    expect(resumeFrom(saved(4))).toBeNull();
    expect(resumeFrom(saved(1400, true))).toBeNull();
  });
});

describe("progressFraction", () => {
  it("fills the bar for a Watched Episode", () => {
    expect(progressFraction(saved(720))).toBe(0.5);
    expect(progressFraction(saved(20, true))).toBe(1);
    expect(progressFraction({ position: 5, duration: 0, watched: false })).toBe(0);
  });
});

describe("continueLabel", () => {
  it("shows the time left, or Up next", () => {
    expect(continueLabel({ position: 720, duration: 1440 })).toBe("12 min left");
    expect(continueLabel({ position: 1430, duration: 1440 })).toBe("1 min left");
    expect(continueLabel({ position: 0, duration: 0 })).toBe("Up next");
  });
});

describe("playAction", () => {
  it("resumes or starts the Up Next Episode of this Show", () => {
    expect(playAction(1, { showId: 1, episode: 4, position: 300, duration: 1440 }, 1)).toEqual({
      episode: 4,
      label: "Resume episode 4",
    });
    expect(playAction(1, { showId: 1, episode: 5, position: 0, duration: 0 }, 1)?.label).toBe("Play episode 5");
  });

  it("uses the first aired Episode when Up Next is in another Show", () => {
    expect(playAction(1, { showId: 9, episode: 1, position: 0, duration: 0 }, 1)).toEqual({ episode: 1, label: "Play episode 1" });
    expect(playAction(1, null, null)).toBeNull();
  });
});
