import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { isActive, streams, STREAMS_KEY, upsertStream, type StreamView } from "./streams";

/**
 * Every Stream of this app session, newest first. The Rust core sends each change as a
 * `stream://update` event; the list also refreshes every 2 s while a Stream is active.
 */
export function useStreams() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: STREAMS_KEY,
    queryFn: streams.list,
    staleTime: Infinity,
    refetchInterval: (q) => (q.state.data?.some(isActive) ? 2000 : false),
  });

  useEffect(() => {
    const unlisteners = [
      listen<StreamView>("stream://update", ({ payload }) =>
        queryClient.setQueryData<StreamView[]>(STREAMS_KEY, (list) => upsertStream(list, payload)),
      ),
      listen<string>("stream://removed", ({ payload }) =>
        queryClient.setQueryData<StreamView[]>(STREAMS_KEY, (list) => list?.filter((s) => s.id !== payload)),
      ),
    ];
    return () => unlisteners.forEach((p) => p.then((off) => off()).catch(() => {}));
  }, [queryClient]);

  return query;
}

/** Stops a Stream and deletes its local data. */
export function useRemoveStream() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: streams.remove,
    onSuccess: (_, id) =>
      queryClient.setQueryData<StreamView[]>(STREAMS_KEY, (list) => list?.filter((s) => s.id !== id)),
  });
}
