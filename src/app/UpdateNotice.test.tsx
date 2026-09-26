// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  install: vi.fn(() => Promise.resolve()),
  relaunch: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));

const { UpdateNotice } = await import("./UpdateNotice");

function renderNotice(enabled = true) {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <UpdateNotice enabled={enabled} />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("UpdateNotice", () => {
  it("installs the new version and restarts", async () => {
    mocks.check.mockResolvedValue({ version: "0.2.0", downloadAndInstall: mocks.install });
    renderNotice();
    fireEvent.click(await screen.findByRole("button", { name: "Update to 0.2.0" }));
    await waitFor(() => expect(mocks.relaunch).toHaveBeenCalled());
    expect(mocks.install).toHaveBeenCalledBefore(mocks.relaunch);
  });

  it("shows nothing when Lokii is up to date", async () => {
    mocks.check.mockResolvedValue(null);
    const { container } = renderNotice();
    await waitFor(() => expect(mocks.check).toHaveBeenCalled());
    expect(container.innerHTML).toBe("");
  });

  it("does not ask for updates in dev builds", () => {
    renderNotice(false);
    expect(mocks.check).not.toHaveBeenCalled();
  });
});
