import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { GitPullButton } from "./GitPullButton.js";

vi.mock("../api/client.js", () => ({
  fetchRepository: vi.fn(),
  fetchMetrics: vi.fn(),
  fetchConfig: vi.fn(),
  fetchTeam: vi.fn(),
  fetchStory: vi.fn(),
  fetchEpic: vi.fn(),
  createSprint: vi.fn(),
  updateSprint: vi.fn(),
  updateStory: vi.fn(),
  planStory: vi.fn(),
  moveStory: vi.fn(),
  updateEpicFields: vi.fn(),
  updateStoryFields: vi.fn(),
  updateTaskStatus: vi.fn(),
  gitPull: vi.fn(),
}));

import { gitPull } from "../api/client.js";

function wrapper({ children }: { children: ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

afterEach(() => vi.restoreAllMocks());

describe("GitPullButton", () => {
  it("renders with pull latest data label", () => {
    render(<GitPullButton />, { wrapper });
    expect(screen.getByRole("button", { name: /pull latest data/i })).toBeInTheDocument();
  });

  it("shows success state after a successful pull", async () => {
    vi.mocked(gitPull).mockResolvedValueOnce({ ok: true, status: "success", message: "Already up to date." });
    render(<GitPullButton />, { wrapper });
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(screen.getByRole("button", { name: /up to date/i })).toBeInTheDocument());
  });

  it("shows error state after a failed pull", async () => {
    vi.mocked(gitPull).mockResolvedValueOnce({ ok: false, status: "error", message: "merge conflict" });
    render(<GitPullButton />, { wrapper });
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /sync failed/i })).toBeInTheDocument(),
    );
  });

  it("shows error state on network failure", async () => {
    vi.mocked(gitPull).mockRejectedValueOnce(new Error("network error"));
    render(<GitPullButton />, { wrapper });
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /sync failed/i })).toBeInTheDocument(),
    );
  });

  it("is disabled while loading", async () => {
    let resolve!: (v: { ok: boolean; status: "success"; message: string }) => void;
    vi.mocked(gitPull).mockReturnValueOnce(new Promise((r) => { resolve = r; }));
    render(<GitPullButton />, { wrapper });
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(screen.getByRole("button")).toBeDisabled());
    act(() => resolve({ ok: true, status: "success", message: "ok" }));
  });
});
