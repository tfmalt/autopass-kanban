import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ProjectProgress } from "./ProjectProgress.js";

describe("ProjectProgress", () => {
  it("renders points and percent", () => {
    render(
      <ProjectProgress
        progress={{ donePoints: 142, totalPoints: 487, doneStories: 63, totalStories: 198, phases: [] }}
      />,
    );
    expect(screen.getByText(/142 \/ 487 pts/)).toBeInTheDocument();
    expect(screen.getByText(/29%/)).toBeInTheDocument();
  });
});
