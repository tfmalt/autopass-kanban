import { describe, expect, it } from "vitest";
import { buildHierarchy, buildWbsRows } from "./wbs.js";
import { computeEstimates } from "./estimates.js";
import { story, repository, metrics } from "./fixtures.js";

describe("buildHierarchy", () => {
  it("groups stories into phase → epic → story", () => {
    const repo = repository();
    const hierarchy = buildHierarchy(repo);
    expect(hierarchy).toHaveLength(1);
    expect(hierarchy[0]!.id).toBe("F1");
    expect(hierarchy[0]!.epics).toHaveLength(1);
    expect(hierarchy[0]!.epics[0]!.id).toBe("EP-F1-01");
    expect(hierarchy[0]!.epics[0]!.stories).toHaveLength(2);
  });

  it("sorts phases, epics, and stories by id", () => {
    const s1 = story({ id: "US-F1-001", title: "A", status: "done", storyPoints: 3, phase: "F1", epic: "EP-F1-01" });
    const s2 = story({ id: "US-F1-002", title: "B", status: "todo", storyPoints: 5, phase: "F1", epic: "EP-F1-01" });
    const repo = repository({ stories: [s2, s1] });
    const hierarchy = buildHierarchy(repo);
    const storyIds = hierarchy[0]!.epics[0]!.stories.map((s) => s.id);
    expect(storyIds).toEqual(["US-F1-001", "US-F1-002"]);
  });
});

describe("buildWbsRows", () => {
  it("produces phase → epic → story rows with correct WBS numbering", () => {
    const repo = repository();
    const m = metrics();
    const { estimates, hoursPerPoint } = computeEstimates(repo.stories, m, repo);
    const rows = buildWbsRows(repo, estimates, hoursPerPoint);

    const phaseRow = rows.find((r) => r.kind === "phase");
    const epicRow = rows.find((r) => r.kind === "epic");
    const storyRows = rows.filter((r) => r.kind === "story");

    expect(phaseRow).toBeDefined();
    expect(phaseRow!.wbs).toBe("1");
    expect(epicRow).toBeDefined();
    expect(epicRow!.wbs).toBe("1.1");
    expect(storyRows[0]!.wbs).toBe("1.1.1");
    expect(storyRows[1]!.wbs).toBe("1.1.2");
  });

  it("rolls up points to phase and epic rows", () => {
    const repo = repository();
    const m = metrics();
    const { estimates, hoursPerPoint } = computeEstimates(repo.stories, m, repo);
    const rows = buildWbsRows(repo, estimates, hoursPerPoint);

    const phaseRow = rows.find((r) => r.kind === "phase")!;
    const epicRow = rows.find((r) => r.kind === "epic")!;

    // Total points for F1 = 5 + 8 = 13
    expect(phaseRow.points).toBe(13);
    expect(epicRow.points).toBe(13);
  });

  it("uses STATUS_LABELS for story status", () => {
    const repo = repository();
    const m = metrics();
    const { estimates, hoursPerPoint } = computeEstimates(repo.stories, m, repo);
    const rows = buildWbsRows(repo, estimates, hoursPerPoint);

    const doneStory = rows.find((r) => r.id === "US-F1-001")!;
    const todoStory = rows.find((r) => r.id === "US-F1-002")!;
    expect(doneStory.status).toBe("DONE");
    expect(todoStory.status).toBe("TODO");
  });

  it("includes phase metadata from PHASE_META", () => {
    const repo = repository();
    const m = metrics();
    const { estimates, hoursPerPoint } = computeEstimates(repo.stories, m, repo);
    const rows = buildWbsRows(repo, estimates, hoursPerPoint);

    const phaseRow = rows.find((r) => r.kind === "phase")!;
    expect(phaseRow.milestone).toBe("MP1 - Foundation");
    expect(phaseRow.period).toBe("Q2 2026");
    expect(phaseRow.priority).toBe("Critical");
  });

  it("includes sprint and assignee in story notes", () => {
    const repo = repository();
    const m = metrics();
    const { estimates, hoursPerPoint } = computeEstimates(repo.stories, m, repo);
    const rows = buildWbsRows(repo, estimates, hoursPerPoint);

    const todoRow = rows.find((r) => r.id === "US-F1-002")!;
    expect(todoRow.notes).toContain("Sprint S001.next");
    expect(todoRow.notes).toContain("Assignee Test User <test@example.com>");
  });

  it("handles multiple phases with sequential WBS phase numbers", () => {
    const s1 = story({ id: "US-F1-001", title: "F1 story", status: "done", storyPoints: 3, phase: "F1", epic: "EP-F1-01" });
    const s2 = story({ id: "US-F2-001", title: "F2 story", status: "todo", storyPoints: 5, phase: "F2", epic: "EP-F2-01" });
    const repo = {
      ...repository(),
      stories: [s1, s2],
      epics: [
        { id: "EP-F1-01", title: "Platform F1", phase: "F1", priority: null, stories: [s1] },
        { id: "EP-F2-01", title: "Platform F2", phase: "F2", priority: null, stories: [s2] },
      ],
    };
    const m = metrics();
    const { estimates, hoursPerPoint } = computeEstimates([s1, s2], m, repo);
    const rows = buildWbsRows(repo, estimates, hoursPerPoint);

    const phaseRows = rows.filter((r) => r.kind === "phase");
    expect(phaseRows[0]!.wbs).toBe("1");
    expect(phaseRows[1]!.wbs).toBe("2");
  });
});
