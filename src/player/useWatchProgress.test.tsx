// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { INITIAL_PLAYER, type PlayerState } from "../lib/player";
import type { ShowLibrary } from "../lib/library";

const mocks = vi.hoisted(() => ({
  show: vi.fn<(showId: number) => Promise<ShowLibrary>>(),
  save: vi.fn((..._args: number[]) => Promise.resolve(false)),
  seek: vi.fn((_seconds: number) => Promise.resolve()),
}));

vi.mock("../lib/library", async (original) => ({
  ...(await original<typeof import("../lib/library")>()),
  library: { show: mocks.show, saveProgress: mocks.save },
}));
vi.mock("../lib/player", async (original) => ({
  ...(await original<typeof import("../lib/player")>()),
  player: { seek: mocks.seek },
}));

const { useWatchProgress } = await import("./useWatchProgress");

const SAVED: ShowLibrary = {
  progress: [{ showId: 1, episode: 3, position: 600, duration: 1440, watched: false, updatedAt: 0 }],
  upNext: null,
  onWatchlist: false,
};

type Props = { state: PlayerState; loaded: boolean };

function setup(initial: Props) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(({ state, loaded }: Props) => useWatchProgress(1, 3, state, loaded), { wrapper, initialProps: initial });
}

const at = (timePos: number): PlayerState => ({ ...INITIAL_PLAYER, timePos, duration: 1440 });

beforeEach(() => {
  mocks.show.mockResolvedValue(SAVED);
  mocks.save.mockClear();
  mocks.seek.mockClear();
});
afterEach(cleanup);

describe("useWatchProgress", () => {
  it("resumes once, after the file loads", async () => {
    const hook = setup({ state: at(0), loaded: false });
    await waitFor(() => expect(mocks.show).toHaveBeenCalled());
    expect(mocks.seek).not.toHaveBeenCalled();
    hook.rerender({ state: at(0), loaded: true });
    await waitFor(() => expect(mocks.seek).toHaveBeenCalledWith(600));
    hook.rerender({ state: at(601), loaded: true });
    expect(mocks.seek).toHaveBeenCalledTimes(1);
  });

  it("saves every 5 seconds from 10 seconds, and the last position on close", async () => {
    const hook = setup({ state: at(0), loaded: true });
    await waitFor(() => expect(mocks.seek).toHaveBeenCalled());
    for (const time of [4, 600, 602, 606, 608]) hook.rerender({ state: at(time), loaded: true });
    expect(mocks.save.mock.calls.map((call) => call[2])).toEqual([600, 606]);
    hook.unmount();
    expect(mocks.save).toHaveBeenLastCalledWith(1, 3, 608, 1440);
  });

  it("saves nothing when the saved position cannot load", async () => {
    mocks.show.mockRejectedValue("offline");
    const hook = setup({ state: at(0), loaded: true });
    await waitFor(() => expect(mocks.show).toHaveBeenCalled());
    hook.rerender({ state: at(30), loaded: true });
    hook.unmount();
    expect(mocks.save).not.toHaveBeenCalled();
    expect(mocks.seek).not.toHaveBeenCalled();
  });

  it("resumes the last position again when a new file loads after Retry", async () => {
    const hook = setup({ state: at(0), loaded: true });
    await waitFor(() => expect(mocks.seek).toHaveBeenCalledWith(600));
    hook.rerender({ state: at(700), loaded: true });
    hook.rerender({ state: at(0), loaded: false });
    hook.rerender({ state: at(0), loaded: true });
    await waitFor(() => expect(mocks.seek).toHaveBeenLastCalledWith(700));
    expect(mocks.save.mock.calls.every((call) => call[2] >= 600)).toBe(true);
  });

  it("marks the Episode Watched for Next Episode", async () => {
    const hook = setup({ state: at(0), loaded: true });
    await act(() => hook.result.current.markWatched(1440));
    expect(mocks.save).toHaveBeenCalledWith(1, 3, 1440, 1440);
  });
});
