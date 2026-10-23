import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { report } from "../report/fixtures.js";
import { groupLeveransestier, LeveransestierView } from "./LeveransestierView.js";

function fixture() {
  const data = report();
  const base = data.wbsRows.find((row) => row.kind === "story")!;
  data.wbsRows = [
    { ...base, id: "US-8", title: "H1-08 - File Generation - Generate TIF", status: "TODO" },
    { ...base, id: "US-3A", title: "H1-03A - Identification - Identify OBE", status: "IN PROGRESS" },
    { ...base, id: "US-X", title: "H1-X01 - Contracts - Define handoffs", status: "DONE" },
    { ...base, id: "US-3", title: "H1-03 - Routing - Route passage", status: "DONE" },
    { ...base, id: "US-2", title: "H2-02 - Agreements - Find agreement", status: "DROPPED" },
    { ...base, id: "US-other", title: "Unrelated work", status: "DONE" },
  ];
  return data;
}

describe("Leveransestier", () => {
  it("groups live stories across buckets, orders suffixes, and separates supporting work", () => {
    const paths = groupLeveransestier(fixture().wbsRows);
    expect(paths.map((path) => path.id)).toEqual(["H1", "H2"]);
    expect(paths[0]?.steps.map((step) => step.sequence)).toEqual(["03", "03A", "08"]);
    expect(paths[0]?.support.map((step) => step.sequence)).toEqual(["X01"]);
    expect(paths[0]?.steps[2]?.service).toBe("File Generation");
  });

  it("renders real progress without claiming E2E completion", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
    client.setQueryData(["report"], fixture());
    render(<QueryClientProvider client={client}><LeveransestierView /></QueryClientProvider>);
    expect(await screen.findByRole("heading", { name: "Leveransestier" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /H1/ })).toBeInTheDocument();
    expect(screen.getByText("2 / 4 stories done")).toBeInTheDocument();
    expect(screen.getAllByText("Not verified").length).toBeGreaterThan(0);
    expect(screen.queryByText("Unrelated work")).not.toBeInTheDocument();
    expect(screen.getByText("Supporting work")).toBeInTheDocument();
  });

  it("accepts sparse sequences and dot notation without inventing missing services", () => {
    const data = fixture();
    data.wbsRows = data.wbsRows.slice(0, 1).map((row) => ({ ...row, title: "H12.08 - Pricing - Price passage" }));
    const paths = groupLeveransestier(data.wbsRows);
    expect(paths).toHaveLength(1);
    expect(paths[0]?.id).toBe("H12");
    expect(paths[0]?.steps).toHaveLength(1);
    expect(paths[0]?.steps[0]?.sequence).toBe("08");
  });
});