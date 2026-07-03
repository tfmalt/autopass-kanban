import { describe, expect, it } from "vitest";
import { estimatesByStory, roundMetric } from "./estimates.js";
import { report } from "./fixtures.js";

describe("server-computed report fixture", () => {
  it("contains WBS rows with phase, epic, and story numbering", () => {
    const rows = report().wbsRows;

    expect(rows.find((row) => row.kind === "phase")?.wbs).toBe("1");
    expect(rows.find((row) => row.kind === "epic")?.wbs).toBe("1.1");
    expect(rows.filter((row) => row.kind === "story").map((row) => row.wbs)).toEqual([
      "1.1.1",
      "1.1.2",
    ]);
  });

  it("carries precomputed estimates and rollups without client arithmetic", () => {
    const data = report();
    const estimates = estimatesByStory(data);

    expect(roundMetric(data.hoursPerPoint)).toBe(2.6);
    expect(estimates.get("US-F1-002")).toEqual({
      estHours: 21,
      estStart: "2026-06-10",
      estEnd: "2026-06-15",
    });
    expect(data.phaseRows[0]).toMatchObject({
      phase: "F1",
      total: 13,
      done: 5,
      remaining: 8,
    });
    expect(data.sprintRows.at(-1)).toMatchObject({
      status: "projected (daily throughput over 3 observed workdays)",
      remaining: 0,
    });
  });
});
