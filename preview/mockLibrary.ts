// Browser preview: Watch Progress and the Watchlist in memory, with a simple Up Next
// (resume the last Episode, else the next one), so the Library screens can be checked.

type Args = Record<string, unknown> | undefined;
type Show = { id: number; episodeList: { number: number; title: string | null; thumbnailUrl: string | null }[] };
type Progress = { showId: number; episode: number; position: number; duration: number; watched: boolean; updatedAt: number };

export function libraryHandlers(showOf: (id: number) => unknown): Record<string, (args: Args) => unknown> {
  const now = Date.now();
  // Attack on Titan: Episodes 1–2 Watched, Episode 3 left at 10 minutes.
  const progress: Progress[] = [
    { showId: 16498, episode: 1, position: 1400, duration: 1440, watched: true, updatedAt: now - 3000 },
    { showId: 16498, episode: 2, position: 1420, duration: 1440, watched: true, updatedAt: now - 2000 },
    { showId: 16498, episode: 3, position: 600, duration: 1440, watched: false, updatedAt: now - 1000 },
  ];
  const watchlist: number[] = [182205];

  const card = (id: number) => {
    const { episodeList: _list, ...rest } = showOf(id) as Show & Record<string, unknown>;
    return rest;
  };
  const upNext = (showId: number) => {
    const rows = progress.filter((p) => p.showId === showId);
    if (rows.length === 0) return null;
    const latest = rows.reduce((a, b) => (b.updatedAt > a.updatedAt ? b : a));
    if (!latest.watched) return { showId, episode: latest.episode, position: latest.position, duration: latest.duration };
    const last = Math.max(...rows.filter((p) => p.watched).map((p) => p.episode));
    return { showId, episode: last + 1, position: 0, duration: 0 };
  };

  return {
    library_save_progress: (args) => {
      const [showId, episode, position, duration] = [args?.showId, args?.episode, args?.position, args?.duration].map(Number);
      const old = progress.find((p) => p.showId === showId && p.episode === episode);
      const watched = (old?.watched ?? false) || position / duration >= 0.9;
      const row = { showId, episode, position, duration, watched, updatedAt: Date.now() };
      if (old) Object.assign(old, row);
      else progress.push(row);
      return watched;
    },
    library_show: (args) => {
      const showId = Number(args?.showId);
      return {
        progress: progress.filter((p) => p.showId === showId).sort((a, b) => a.episode - b.episode),
        upNext: upNext(showId),
        onWatchlist: watchlist.includes(showId),
      };
    },
    library_continue: () =>
      [...new Set([...progress].sort((a, b) => b.updatedAt - a.updatedAt).map((p) => p.showId))].flatMap((showId) => {
        const next = upNext(showId);
        const info = next && (showOf(showId) as Show).episodeList.find((e) => e.number === next.episode);
        if (!next || !info) return [];
        return [{ show: card(showId), episode: next.episode, episodeTitle: info.title, thumbnailUrl: info.thumbnailUrl, position: next.position, duration: next.duration }];
      }),
    library_watchlist: () => watchlist.map(card),
    library_set_watchlist: (args) => {
      const showId = Number(args?.showId);
      const index = watchlist.indexOf(showId);
      if (args?.on && index < 0) watchlist.unshift(showId);
      if (!args?.on && index >= 0) watchlist.splice(index, 1);
    },
  };
}
