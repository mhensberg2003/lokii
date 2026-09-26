// Browser preview: in-memory Settings for TorBox and Local Torrent (see mockTauri.ts).

type Args = Record<string, unknown> | undefined;

const DATA_DIR = "/Users/you/Library/Caches/app.lokii.desktop/lokii-streams";

let apiKey: string | null = null;
let port: number | null = null;
let folder: string | null = null;

function torboxStatus() {
  if (apiKey === null) return { connected: false, keyHint: null, account: null, error: null };
  return {
    connected: true,
    keyHint: apiKey.slice(-4),
    account: { plan: "Pro", email: "you@example.com", premiumExpiresAt: "2026-12-01T00:00:00Z" },
    error: null,
  };
}

function localSettings() {
  return { port, folder, dataDir: folder ? `${folder}/lokii-streams` : DATA_DIR, restartNeeded: false };
}

export const settingsHandlers: Record<string, (args: Args) => unknown> = {
  torbox_status: () => torboxStatus(),
  torbox_connect: (args) => {
    const key = String(args?.apiKey ?? "").trim();
    if (key.length < 8) return Promise.reject("TorBox did not accept the API key.");
    apiKey = key;
    return torboxStatus();
  },
  torbox_disconnect: () => {
    apiKey = null;
    return null;
  },
  local_settings: () => localSettings(),
  local_set_settings: (args) => {
    port = (args?.port as number | null) ?? null;
    folder = (args?.folder as string | null) ?? null;
    return localSettings();
  },
  "plugin:dialog|open": () => "/Users/you/Movies",
  "plugin:app|version": () => "0.1.0",
};
