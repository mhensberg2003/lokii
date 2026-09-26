import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { library, LIBRARY_KEY, libraryKeys, RESUME_MIN, resumeFrom, SAVE_EVERY } from "../lib/library";
import { player, type PlayerState } from "../lib/player";

type Position = { position: number; duration: number };

/**
 * Resumes the Episode from its saved Watch Progress, then saves the position every
 * few seconds and once more when the player closes. Nothing is saved until the saved
 * position is known, so a failed read never replaces it. `markWatched` is for Next Episode.
 */
export function useWatchProgress(showId: number, episode: number, state: PlayerState, fileLoaded: boolean) {
  const queryClient = useQueryClient();
  const saved = useQuery({ queryKey: libraryKeys.show(showId), queryFn: () => library.show(showId), staleTime: 0 });
  const resumed = useRef(false);
  const savedAt = useRef<number | null>(null);
  const latest = useRef<Position | null>(null);

  // A new file (the first one, or a new one after Retry) starts at 0: resume it again.
  useEffect(() => {
    if (fileLoaded) return;
    resumed.current = false;
    savedAt.current = null;
  }, [fileLoaded]);

  useEffect(() => {
    if (resumed.current || !fileLoaded || !saved.isSuccess) return;
    resumed.current = true;
    const from = latest.current?.position ?? resumeFrom(saved.data.progress.find((p) => p.episode === episode));
    if (from !== null) player.seek(from).catch(() => {});
  }, [fileLoaded, saved.isSuccess, saved.data, episode]);

  useEffect(() => {
    if (!resumed.current || !fileLoaded || state.duration <= 0 || state.timePos < RESUME_MIN) return;
    latest.current = { position: state.timePos, duration: state.duration };
    if (savedAt.current !== null && Math.abs(state.timePos - savedAt.current) < SAVE_EVERY) return;
    savedAt.current = state.timePos;
    library.saveProgress(showId, episode, state.timePos, state.duration).catch(() => {});
  }, [showId, episode, fileLoaded, state.timePos, state.duration]);

  useEffect(
    () => () => {
      const last = latest.current;
      const save = last ? library.saveProgress(showId, episode, last.position, last.duration) : Promise.resolve(false);
      save.catch(() => {}).finally(() => void queryClient.invalidateQueries({ queryKey: LIBRARY_KEY }));
    },
    [showId, episode, queryClient],
  );

  /** Saves the Episode as played to the end. Resolves when it is saved. */
  const markWatched = async (duration: number) => {
    if (duration <= 0) return;
    latest.current = { position: duration, duration };
    await library.saveProgress(showId, episode, duration, duration).catch(() => false);
  };
  return { markWatched };
}
