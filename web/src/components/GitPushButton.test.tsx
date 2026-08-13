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
  it("requires a pull before pushing when the remote is ahead", () => {
    render(
      <GitPushButton
        status={{ available: true, upstream: "origin/main", ahead: 0, behind: 1, pendingCount: 1 }}
      />,
      { wrapper },
    );

    expect(screen.getByRole("button", { name: /pull latest data before pushing/i })).toBeDisabled();
  });
});
