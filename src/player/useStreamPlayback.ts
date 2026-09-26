import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { player, type PlayerState } from "../lib/player";
import { streams, STREAMS_KEY, upsertStream, WATCHED_AT, type StreamView } from "../lib/streams";
import { useStreams } from "../lib/useStreams";

/**
 * Starts the Stream for the Episode, loads its URL into mpv, and reports Watched.
 * `fileLoaded` is false until mpv has loaded this Episode's file; until then mpv's time
 * and length can still belong to the previous file.
 */
export function useStreamPlayback(showId: number, episode: number, state: PlayerState) {
  const queryClient = useQueryClient();
  const [streamId, setStreamId] = useState<string | null>(null);
  const [startError, setStartError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const stream = useStreams().data?.find((s) => s.id === streamId) ?? null;
  const loaded = useRef<string | null>(null);
  const [fileLoaded, setFileLoaded] = useState(false);
  const watched = useRef(false);
  const current = useRef<string | null>(null);
  current.current = streamId;

  // Leaving the player in any way (Back, history, Next Episode) stops mpv and closes the Stream.
  useEffect(
    () => () => {
      player.stop().catch(() => {});
      if (current.current) streams.close(current.current).catch(() => {});
    },
    [],
  );

  useEffect(() => {
    setStartError("");
    streams
      .start(showId, episode)
      .then((view) => {
        queryClient.setQueryData<StreamView[]>(STREAMS_KEY, (list) => upsertStream(list, view));
        setStreamId(view.id);
      })
      .catch((err) => setStartError(String(err)));
  }, [showId, episode, attempt, queryClient]);

  useEffect(() => {
    const unlisten = listen("player://file-loaded", () => {
      if (loaded.current) setFileLoaded(true);
    });
    return () => void unlisten.then((off) => off()).catch(() => {});
  }, []);

  const url = stream?.phase.kind === "ready" ? stream.url : null;
  useEffect(() => {
    if (!url || !state.ready || loaded.current === url) return;
    loaded.current = url;
    player.load(url).catch(() => {});
  }, [url, state.ready]);

  useEffect(() => {
    if (!stream || !fileLoaded || watched.current || state.duration <= 0) return;
    if (state.timePos / state.duration >= WATCHED_AT) {
      watched.current = true;
      streams.watched(stream.id).catch(() => {
        watched.current = false;
      });
    }
  }, [stream, fileLoaded, state.timePos, state.duration]);

  return { stream, fileLoaded, startError, retry: () => setAttempt((n) => n + 1) };
}
