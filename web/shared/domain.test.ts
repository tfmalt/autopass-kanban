import { describe, expect, it } from "vitest";
import { slugifyHeadline } from "./domain.js";

describe("slugifyHeadline", () => {
  // Keep these cases aligned with crates/core/src/util.rs tests.
  it("matches Rust slugify_headline parity cases", () => {
    expect(slugifyHeadline("Foundation Sprint!")).toBe("foundation-sprint");
    expect(slugifyHeadline("  Alpha   Beta  ")).toBe("alpha-beta");
    expect(slugifyHeadline("Roadmap_2026")).toBe("roadmap-2026");
    expect(slugifyHeadline("---")).toBe("");
  });
});
