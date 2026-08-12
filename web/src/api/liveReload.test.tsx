/**
 * WP-05 guards: bounded query staleness, live-reload degradation, and scoped
 * invalidation.
 *
 * These cover the failure mode the previous revision of the loading plan would
 * have shipped: making SSE the sole freshness mechanism, so that any lost
 * stream left the UI permanently and silently stale.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./client.js", () => ({
  fetchRepository: vi.fn(),
  fetchMetrics: vi.fn(),
  fetchReport: vi.fn(),
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

import { fetchRepository, gitPull } from "./client.js";
import { useGitPull, useLiveReload, useRepository } from "./hooks.js";

// ---------------------------------------------------------------------------
// EventSource stub — the jsdom environment provides none.
// ---------------------------------------------------------------------------
type Listener = (event: Event) => void;

class EventSourceStub {
  static instances: EventSourceStub[] = [];
  readonly listeners = new Map<string, Set<Listener>>();
  closed = false;

  constructor(readonly url: string) {
    EventSourceStub.instances.push(this);
  }

  addEventListener(type: string, listener: Listener) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(listener);
  }

  removeEventListener(type: string, listener: Listener) {
    this.listeners.get(type)?.delete(listener);
  }

  close() {
    this.closed = true;
  }

  emit(type: string) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(new Event(type));
    }
  }
}

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

/**
 * Mirrors the production defaults in `main.tsx`. If those change, this test
 * client must change with them or the assertions below stop meaning anything.
 */
function productionLikeClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 60_000,
        refetchOnWindowFocus: true,
        refetchOnMount: false,
        refetchOnReconnect: true,
        retry: false,
      },
    },
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  EventSourceStub.instances = [];
  vi.stubGlobal("EventSource", EventSourceStub);
  // `requestAnimationFrame` batches invalidation; run callbacks synchronously so
  // the tests do not depend on frame timing.
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("query freshness policy", () => {
  it("serves a remount from cache inside the stale window", async () => {
    const qc = productionLikeClient();
    vi.mocked(fetchRepository).mockResolvedValue({ stories: [] } as never);

    const first = renderHook(() => useRepository(), { wrapper: wrapper(qc) });
    await waitFor(() => expect(first.result.current.isSuccess).toBe(true));
    expect(fetchRepository).toHaveBeenCalledTimes(1);
    first.unmount();

    const second = renderHook(() => useRepository(), { wrapper: wrapper(qc) });
    await waitFor(() => expect(second.result.current.isSuccess).toBe(true));
    expect(
      fetchRepository,
      "a remount within staleTime must not refetch",
    ).toHaveBeenCalledTimes(1);
  });

  it("refetches once the data is stale", async () => {
    const qc = productionLikeClient();
    vi.mocked(fetchRepository).mockResolvedValue({ stories: [] } as never);

    const view = renderHook(() => useRepository(), { wrapper: wrapper(qc) });
    await waitFor(() => expect(view.result.current.isSuccess).toBe(true));

    // Simulate the stale window elapsing.
    qc.getQueryCache().find({ queryKey: ["repository"] })!.state.dataUpdatedAt =
      Date.now() - 120_000;
    await act(async () => {
      await view.result.current.refetch();
    });
    expect(fetchRepository).toHaveBeenCalledTimes(2);
  });
});

describe("useLiveReload", () => {
  it("invalidates each aggregate key once per change event", async () => {
    const qc = productionLikeClient();
    const invalidate = vi.spyOn(qc, "invalidateQueries");

    renderHook(() => useLiveReload(), { wrapper: wrapper(qc) });
    const source = EventSourceStub.instances.at(-1)!;

    await act(async () => {
      source.emit("change");
    });

    const keys = invalidate.mock.calls.map(([filters]) =>
      JSON.stringify((filters as { queryKey: unknown }).queryKey),
    );
    expect(keys).toEqual([
      '["repository"]',
      '["metrics"]',
      '["report"]',
      '["team"]',
    ]);
  });

  it("reports the degraded state and starts polling when the stream errors", async () => {
    vi.useFakeTimers();
    try {
      const qc = productionLikeClient();
      const invalidate = vi.spyOn(qc, "invalidateQueries");
      const { result } = renderHook(() => useLiveReload(), { wrapper: wrapper(qc) });
      expect(result.current.connected).toBe(true);

      const source = EventSourceStub.instances.at(-1)!;
      act(() => {
        source.emit("error");
      });
      expect(
        result.current.connected,
        "losing the stream must be visible, not silent",
      ).toBe(false);

      invalidate.mockClear();
      act(() => {
        vi.advanceTimersByTime(30_000);
      });
      expect(
        invalidate.mock.calls.length,
        "the polling fallback must keep the aggregates fresh while SSE is down",
      ).toBeGreaterThan(0);

      // Recovery clears both the indicator and the fallback poll.
      invalidate.mockClear();
      act(() => {
        source.emit("open");
      });
      expect(result.current.connected).toBe(true);
      act(() => {
        vi.advanceTimersByTime(60_000);
      });
      expect(
        invalidate,
        "polling must stop once the stream recovers",
      ).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("closes the stream on unmount", () => {
    const qc = productionLikeClient();
    const { unmount } = renderHook(() => useLiveReload(), { wrapper: wrapper(qc) });
    const source = EventSourceStub.instances.at(-1)!;
    unmount();
    expect(source.closed).toBe(true);
  });
});

describe("useGitPull", () => {
  it("invalidates aggregates and the git sync status", async () => {
    const qc = productionLikeClient();
    qc.setQueryData(["config"], { version: "test" });
    qc.setQueryData(["story", "US-001"], { id: "US-001" });
    const invalidate = vi.spyOn(qc, "invalidateQueries");
    vi.mocked(gitPull).mockResolvedValue({ ok: true } as never);

    const { result } = renderHook(() => useGitPull(), { wrapper: wrapper(qc) });
    await act(async () => {
      await result.current.mutateAsync();
    });

    const keys = invalidate.mock.calls.map(([filters]) =>
      JSON.stringify((filters as { queryKey: unknown } | undefined)?.queryKey),
    );
    expect(keys).toEqual([
      '["repository"]',
      '["metrics"]',
      '["report"]',
      '["team"]',
      '["gitStatus"]',
    ]);
    expect(
      keys,
      "an unfiltered invalidateQueries() would also discard config and story detail",
    ).not.toContain(undefined);
  });
});
