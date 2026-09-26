// Browser preview: replaces the Tauri bridge with saved AniList responses, so the UI can be
// checked in a normal browser (`pnpm preview:ui`). Refresh the data with:
//   cd src-tauri && cargo test dump_preview_fixtures -- --ignored
//   cd src-tauri && cargo test live_episode_releases -- --ignored

import { settingsHandlers } from "./mockSettings";
import { playerHandlers } from "./mockPlayer";
import { streamHandlers } from "./mockStreams";

const fixtures = import.meta.glob<unknown>("./fixtures/*.json", { eager: true, import: "default" });

function fixture(name: string): unknown {
  const value = fixtures[`./fixtures/${name}.json`];
  if (value === undefined) throw new Error(`No preview fixture "${name}"`);
  return value;
}

function hasFixture(name: string): boolean {
  return `./fixtures/${name}.json` in fixtures;
}

type Args = Record<string, unknown> | undefined;
type Callback = (message: unknown) => void;

/** Callbacks by ID (transformCallback), and listener IDs by event name. */
const callbacks = new Map<number, Callback>();
const listeners = new Map<string, Set<number>>();

function emit(event: string, payload: unknown) {
  for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
}

const handlers: Record<string, (args: Args) => unknown> = {
  ...settingsHandlers,
  ...streamHandlers(fixtures),
  ...playerHandlers(emit),
  catalog_home: () => fixture("home"),
  catalog_browse: (args) => {
    const genre = String(args?.genre);
    const feed = fixture("browse-Action") as { rows: unknown[] };
    return hasFixture(`browse-${genre}`) ? fixture(`browse-${genre}`) : { ...feed, genre };
  },
  catalog_show: (args) => {
    const id = Number(args?.id);
    if (hasFixture(`show-${id}`)) return fixture(`show-${id}`);
    const first = Object.keys(fixtures).find((key) => key.includes("/show-"));
    return first ? fixtures[first] : Promise.reject("No show fixtures");
  },
  catalog_search: () => fixture("search"),
  index_releases: (args) => episodeReleases(Number(args?.showId), Number(args?.episode)),
  index_pick: (args) => {
    const showId = Number(args?.showId);
    const hash = (args?.infoHash as string | null) ?? null;
    if (hash === null) picks.delete(showId);
    else picks.set(showId, hash);
    return episodeReleases(showId, Number(args?.episode));
  },
  "plugin:event|listen": (args) => {
    const event = String(args?.event);
    const id = Number(args?.handler);
    listeners.set(event, (listeners.get(event) ?? new Set()).add(id));
    return id;
  },
  "plugin:event|unlisten": (args) => {
    listeners.get(String(args?.event))?.delete(Number(args?.eventId));
  },
};

type PreviewReleases = { releases: { infoHash: string }[]; chosen: string | null; pickedByUser: boolean };

/** Picks made in this preview session, by Show. */
const picks = new Map<number, string>();

/** Serves the saved Releases of a Show for any Episode, with the preview pick applied. */
function episodeReleases(showId: number, episode: number): unknown {
  const key = Object.keys(fixtures).find((k) => k.startsWith(`./fixtures/releases-${showId}-`));
  if (!key) return { showId, episode, releases: [], chosen: null, pickedByUser: false };
  const saved = fixtures[key] as PreviewReleases;
  const pick = picks.get(showId);
  const picked = pick !== undefined && saved.releases.some((r) => r.infoHash === pick);
  return { ...saved, showId, episode, chosen: picked ? pick : saved.chosen, pickedByUser: picked };
}

let callbackId = 0;

(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke: async (command: string, args: Args) => {
    const handler = handlers[command];
    if (!handler) throw `"${command}" is not available in the browser preview`;
    // A short delay so loading states are visible, like the real app.
    await new Promise((resolve) => setTimeout(resolve, 150));
    return handler(args);
  },
  transformCallback: (callback: Callback) => {
    callbacks.set(++callbackId, callback);
    return callbackId;
  },
  unregisterCallback: (id: number) => callbacks.delete(id),
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
};

(window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__: unknown }).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
  unregisterListener: (event: string, id: number) => listeners.get(event)?.delete(id),
};

// A still "video frame" behind the transparent player page. Other routes paint over it.
document.documentElement.style.background = "radial-gradient(circle at 30% 35%, #4a5a78, #1a1d27 55%, #0c0d12)";
