import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchEpic, fetchRepository, gitPull, moveStory, updateEpicFields, updateStoryFields } from "./client.js";

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

  it("gitPull returns success response", async () => {
    const payload = { ok: true, status: "success", message: "Already up to date.", pulledAt: "2026-06-30T12:00:00Z" };
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(payload), { status: 200 })));
    const result = await gitPull();
    expect(result.ok).toBe(true);
    expect(result.status).toBe("success");
  });

  it("gitPull returns error response on non-OK status", async () => {
    const payload = { ok: false, status: "error", message: "git pull failed: conflict" };
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(payload), { status: 200 })));
    const result = await gitPull();
    expect(result.ok).toBe(false);
    expect(result.status).toBe("error");
  });

  it("gitPull throws on HTTP error", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ error: "server error" }), { status: 500 })));
    await expect(gitPull()).rejects.toThrow("server error");
  });

  it("gitPull POSTs to /api/git-pull", async () => {
    const fetchMock = vi.fn(async () =>
      new Response(JSON.stringify({ ok: true, status: "success", message: "Already up to date." }), { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchMock);
    await gitPull();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/git-pull",
      expect.objectContaining({ method: "POST" }),
    );
  });
});
