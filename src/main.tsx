import React from "react";
import ReactDOM from "react-dom/client";
import { createHashRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AppShell } from "./app/AppShell";
import { Home } from "./routes/Home";
import { Browse } from "./routes/Browse";
import { Show } from "./routes/Show";
import { Search } from "./routes/Search";
import { Player } from "./routes/Player";
import { Downloads, Library, Settings } from "./routes/Placeholders";
import "./styles/base.css";

// Browser preview only: swaps the Tauri bridge for saved data (see preview/mockTauri.ts).
if (import.meta.env.VITE_PREVIEW === "1") {
  await import("../preview/mockTauri");
}

if (/Mac/.test(navigator.userAgent)) {
  document.documentElement.classList.add("platform-mac");
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // The Rust core caches AniList data; the UI only keeps it for the session.
      staleTime: 5 * 60 * 1000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

const router = createHashRouter([
  {
    Component: AppShell,
    children: [
      { index: true, Component: Home },
      { path: "browse", Component: Browse },
      { path: "show/:id", Component: Show },
      { path: "search", Component: Search },
      { path: "library", Component: Library },
      { path: "downloads", Component: Downloads },
      { path: "settings", Component: Settings },
    ],
  },
  // The player has no shell: the transparent page shows mpv's video layer.
  { path: "player", Component: Player },
]);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </React.StrictMode>,
);
