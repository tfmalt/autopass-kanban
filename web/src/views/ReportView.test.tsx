import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { report } from "../report/fixtures.js";
import { ReportView } from "./ReportView.js";

function renderWithClient(ui: ReactNode) {
  const qc = new QueryClient();
  qc.setQueryData(["report"], report());
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe("ReportView", () => {
  it("renders WBS, phase summary, and sprint prognosis from live data", async () => {
    renderWithClient(<ReportView />);

    expect(await screen.findByRole("heading", { name: "WBS Report" })).toBeInTheDocument();
    expect(screen.getByText("US-F1-001")).toBeInTheDocument();
    expect(screen.getByText("Todo story")).toBeInTheDocument();
    expect(screen.getAllByText("MP1 - Foundation").length).toBeGreaterThan(0);
    expect(screen.getByText(/P50 2026-06-15 \/ P80 2026-06-16 \/ P90 2026-06-17/)).toBeInTheDocument();
    expect(screen.getByText("S000.start")).toBeInTheDocument();
    expect(screen.getAllByText(/daily throughput over 3 observed workdays/).length).toBeGreaterThan(0);
  });
});
