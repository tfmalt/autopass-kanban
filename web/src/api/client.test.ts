import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchEpic, fetchRepository, moveStory, updateEpicFields, updateStoryFields } from "./client.js";

afterEach(() => vi.restoreAllMocks());

describe("api client", () => {
  it("fetchRepository returns parsed JSON", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        new Response(
          JSON.stringify({
            stories: [],
            sprints: [],
            epics: [],
            progress: { donePoints: 1, totalPoints: 2, doneStories: 0, totalStories: 0, phases: [] },
          }),
          { status: 200 },
        ),
      ),
    );
    const repo = await fetchRepository();
    expect(repo.progress.totalPoints).toBe(2);
  });

  it("moveStory throws on non-OK response with the server error", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ error: "boom" }), { status: 422 })));
    await expect(moveStory("US-F1-001", "done")).rejects.toThrow("boom");
  });

  it("fetchEpic returns parsed JSON", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify({ id: "EP-F1-01", title: "Platform", phase: "F1", stories: [], body: "# Epic: Platform" }), { status: 200 })),
    );
    const epic = await fetchEpic("EP-F1-01");
    expect(epic.title).toBe("Platform");
  });

  it("updateStoryFields passes priority", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await updateStoryFields("US-F1-001", { priority: 20 });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/stories/US-F1-001/fields",
      expect.objectContaining({
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ priority: 20 }),
      }),
    );
  });

  it("updateEpicFields POSTs correct body", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ ok: true }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await updateEpicFields("EP-F1-01", { priority: 10 });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/epics/EP-F1-01/fields",
      expect.objectContaining({
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ priority: 10 }),
      }),
    );
  });
});
