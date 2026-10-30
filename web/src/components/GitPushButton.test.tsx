import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { GitPushButton } from "./GitPushButton.js";

vi.mock("../api/client.js", () => ({
  gitPush: vi.fn(),
}));

function wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}

describe("GitPushButton", () => {
  it("does not claim the repository is unavailable before status loads", () => {
    render(<GitPushButton />, { wrapper });
    expect(screen.getByRole("button", { name: "Checking Git status" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: /not a git repository/i })).not.toBeInTheDocument();
  });

  it("allows pushing when the remote is ahead by syncing automatically", () => {
    render(
      <GitPushButton
        status={{ available: true, upstream: "origin/main", ahead: 0, behind: 1, pendingCount: 1 }}
      />,
      { wrapper },
    );

    expect(
      screen.getByRole("button", { name: /commit, sync with remote, and push changes/i }),
    ).not.toBeDisabled();
  });
});
