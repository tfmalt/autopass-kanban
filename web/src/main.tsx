import React, { Suspense, lazy } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createBrowserRouter, Navigate } from "react-router-dom";
import { AppShell } from "./components/AppShell.js";
import { ViewSkeleton } from "./components/Skeletons.js";
import "./styles/tokens.css";
import "./styles/app.css";

const BoardView = lazy(async () => {
  const module = await import("./views/BoardView.js");
  return { default: module.BoardView };
});

const BacklogView = lazy(async () => {
  const module = await import("./views/BacklogView.js");
  return { default: module.BacklogView };
});

const SprintsView = lazy(async () => {
  const module = await import("./views/SprintsView.js");
  return { default: module.SprintsView };
});

const DashboardView = lazy(async () => {
  const module = await import("./views/DashboardView.js");
  return { default: module.DashboardView };
});

const ReportView = lazy(async () => {
  const module = await import("./views/ReportView.js");
  return { default: module.ReportView };
});

/**
 * Server-state freshness policy.
 *
 * Live reload (SSE) is the primary freshness mechanism, but it is not a
 * guarantee: a client can miss changes to a reconnect gap, a lagged broadcast
 * receiver, or the server's subscriber cap. Making SSE the *sole* mechanism —
 * `staleTime: Infinity` with focus and mount refetching disabled — converts each
 * of those into permanent, silent staleness in a tool whose entire value is
 * showing current state.
 *
 * A bounded 60 s staleness plus SSE gives the same practical freshness while
 * degrading gracefully. Now that the server answers in tens of milliseconds, the
 * refetch this costs on window focus is not worth optimizing away.
 */
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      refetchOnWindowFocus: true,
      // A remount inside the stale window is served from cache; SSE and the
      // staleness bound both still apply.
      refetchOnMount: false,
      refetchOnReconnect: true,
    },
  },
});

function RouteFallback() {
  return <ViewSkeleton label="Loading view" />;
}

function withSuspense(element: React.ReactElement) {
  return <Suspense fallback={<RouteFallback />}>{element}</Suspense>;
}

const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to="/board" replace /> },
      { path: "board", element: withSuspense(<BoardView />) },
      { path: "backlog", element: withSuspense(<BacklogView />) },
      { path: "sprints", element: withSuspense(<SprintsView />) },
      { path: "dashboard", element: withSuspense(<DashboardView />) },
      { path: "report", element: withSuspense(<ReportView />) },
    ],
  },
]);

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </React.StrictMode>,
);
