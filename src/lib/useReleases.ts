import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { releases, releasesKey, type EpisodeReleases } from "./releases";

/** The Releases for one Episode. The Index caches on disk too, so ten minutes is plenty. */
export function useEpisodeReleases(showId: number, episode: number) {
  return useQuery({
    queryKey: releasesKey(showId, episode),
    queryFn: () => releases.forEpisode(showId, episode),
    staleTime: 10 * 60 * 1000,
  });
}

/** Saves the user's pick (`null` = automatic) and refreshes the Show's Episodes. */
export function usePickRelease(showId: number, episode: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (infoHash: string | null) => releases.pick(showId, episode, infoHash),
    onSuccess: (data: EpisodeReleases) => {
      queryClient.setQueryData(releasesKey(showId, data.episode), data);
      // A pick applies to the whole Show, so other Episodes must choose again.
      queryClient.invalidateQueries({
        queryKey: ["index", "releases", showId],
        predicate: (query) => query.queryKey[3] !== data.episode,
      });
    },
  });
}
