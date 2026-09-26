// Hooks that read mpv and AniSkip: the observed properties, the tracks, the Skip Segments.

import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useQuery } from "@tanstack/react-query";
import { INITIAL_PLAYER, player, PROPERTY_KEYS, type PlayerState, type Track } from "../lib/player";

function unlistenAll(unlisteners: Promise<UnlistenFn>[]) {
  unlisteners.forEach((p) => p.then((off) => off()).catch(() => {}));
}

function withProperty(state: PlayerState, name: string, value: unknown): PlayerState {
  const key = PROPERTY_KEYS[name];
  return key ? { ...state, [key]: value ?? INITIAL_PLAYER[key] } : state;
}

/** mpv's observed properties, kept in React state. */
export function usePlayerState(): PlayerState {
  const [state, setState] = useState<PlayerState>(INITIAL_PLAYER);
  useEffect(() => {
    const unlisteners = [
      listen("player://ready", () => setState((s) => ({ ...s, ready: true }))),
      listen<{ name: string; value: unknown }>("player://property", ({ payload }) =>
        setState((s) => withProperty(s, payload.name, payload.value)),
      ),
      listen<string>("player://error", ({ payload }) => setState((s) => ({ ...s, error: payload }))),
    ];
    // mpv can start before these listeners exist, so read the current values once as well.
    player
      .snapshot()
      .then((snapshot) =>
        setState((s) =>
          Object.entries(snapshot).reduce<PlayerState>((next, [name, value]) => withProperty(next, name, value), { ...s, ready: true }),
        ),
      )
      .catch(() => {});
    return () => unlistenAll(unlisteners);
  }, []);
  return state;
}

/** The audio and subtitle tracks of the loaded file. Reloads when a file loads or a track changes. */
export function useTracks(sid: string, aid: string): Track[] {
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loads, setLoads] = useState(0);
  useEffect(() => {
    const unlisteners = [listen("player://file-loaded", () => setLoads((n) => n + 1))];
    return () => unlistenAll(unlisteners);
  }, []);
  useEffect(() => {
    let current = true;
    player
      .tracks()
      .then((list) => current && setTracks(list))
      .catch(() => {});
    return () => {
      current = false;
    };
  }, [loads, sid, aid]);
  return tracks;
}

/** The Skip Segments of the Episode, asked for once mpv knows its length. */
export function useSkipSegments(showId: number, episode: number, duration: number) {
  const length = Math.round(duration);
  const query = useQuery({
    queryKey: ["skip", showId, episode, length],
    queryFn: () => player.skipSegments(showId, episode, length),
    enabled: length > 0,
    staleTime: Infinity,
    retry: false,
  });
  return query.data ?? [];
}
