// Browser preview: replaces the Tauri bridge with saved AniList responses, so the UI can be
// checked in a normal browser (`pnpm preview:ui`). Refresh the data with:
//   cd src-tauri && cargo test dump_preview_fixtures -- --ignored

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

const handlers: Record<string, (args: Args) => unknown> = {
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
  "plugin:event|listen": () => 0,
  "plugin:event|unlisten": () => undefined,
};

let callbackId = 0;

(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke: async (command: string, args: Args) => {
    const handler = handlers[command];
    if (!handler) throw `"${command}" is not available in the browser preview`;
    // A short delay so loading states are visible, like the real app.
    await new Promise((resolve) => setTimeout(resolve, 150));
    return handler(args);
  },
  transformCallback: () => ++callbackId,
  unregisterCallback: () => undefined,
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
};
