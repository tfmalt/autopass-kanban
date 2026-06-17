import React, { Suspense, lazy } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createBrowserRouter, Navigate } from "react-router-dom";
import { AppShell } from "./components/AppShell.js";
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

const queryClient = new QueryClient();

function RouteFallback() {
  return <div className="view">Loading...</div>;
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
