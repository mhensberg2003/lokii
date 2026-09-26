// Settings data contract between the Rust core (src-tauri/src/sources) and the UI.
// Terms follow CONTEXT.md: Source, TorBox, Local Torrent.

import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

export type TorBoxAccount = {
  plan: string;
  email: string | null;
  /** ISO date, UTC. */
  premiumExpiresAt: string | null;
};

export type TorBoxStatus = {
  connected: boolean;
  /** The last 4 characters of the API key. */
  keyHint: string | null;
  account: TorBoxAccount | null;
  /** Why the account could not load while the key is still saved. */
  error: string | null;
};

export type LocalSettings = {
  /** `null` lets the OS choose a free port. */
  port: number | null;
  /** `null` uses the app's cache folder. */
  folder: string | null;
  /** Where the torrent data goes with these settings. */
  dataDir: string;
  /** The running engine uses other settings; they apply after a restart. */
  restartNeeded: boolean;
};

export const settings = {
  torboxStatus: () => invoke<TorBoxStatus>("torbox_status"),
  torboxConnect: (apiKey: string) => invoke<TorBoxStatus>("torbox_connect", { apiKey }),
  torboxDisconnect: () => invoke<null>("torbox_disconnect"),
  local: () => invoke<LocalSettings>("local_settings"),
  setLocal: (port: number | null, folder: string | null) =>
    invoke<LocalSettings>("local_set_settings", { port, folder }),
};

export const torboxKey = ["settings", "torbox"] as const;
export const localKey = ["settings", "local"] as const;

export type PortInput = { port: number | null } | { error: string };

/** Reads the port field: empty means automatic, else a whole number from 1024 to 65535. */
export function parsePort(text: string): PortInput {
  const trimmed = text.trim();
  if (trimmed === "") return { port: null };
  const port = Number(trimmed);
  if (!/^\d+$/.test(trimmed) || port < 1024 || port > 65535) {
    return { error: "Use a port from 1024 to 65535, or leave it empty." };
  }
  return { port };
}

/** A masked API key that shows only the last 4 characters. */
export function maskedKey(hint: string | null): string {
  return `••••••••••••${hint ?? ""}`;
}

export function useTorBoxStatus() {
  return useQuery({ queryKey: torboxKey, queryFn: settings.torboxStatus, staleTime: 60 * 1000 });
}

export function useConnectTorBox() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: settings.torboxConnect,
    onSuccess: (status) => queryClient.setQueryData(torboxKey, status),
  });
}

export function useDisconnectTorBox() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: settings.torboxDisconnect,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: torboxKey }),
  });
}

export function useLocalSettings() {
  return useQuery({ queryKey: localKey, queryFn: settings.local });
}

export function useSetLocalSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ port, folder }: { port: number | null; folder: string | null }) =>
      settings.setLocal(port, folder),
    onSuccess: (data) => queryClient.setQueryData(localKey, data),
  });
}
