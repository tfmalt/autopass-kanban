import { NavLink, Outlet } from "react-router-dom";
import { useLiveReload, useConfig, useRepository } from "../api/hooks.js";
import { GitPullButton } from "./GitPullButton.js";
import { ProjectProgress } from "./ProjectProgress.js";

export function AppShell() {
  useLiveReload();
  const repo = useRepository();
  const config = useConfig();
  return (
    <div className="shell">
      <header className="shell-header">
        <div className="brand-block">
          <span className="brand">AutoPASS IP 2.0 · Kanban</span>
          {config.data && (
            <span className="brand-meta">v{config.data.version} · {config.data.branch}</span>
          )}
        </div>
        <nav className="nav">
          <NavLink to="/board">Board</NavLink>
          <NavLink to="/backlog">Backlog</NavLink>
          <NavLink to="/sprints">Sprints</NavLink>
          <NavLink to="/dashboard">Dashboard</NavLink>
          <NavLink to="/report">Report</NavLink>
        </nav>
        <span className="spacer" />
        {repo.data && <ProjectProgress progress={repo.data.progress} />}
        <GitPullButton />
      </header>
      <main>
        <Outlet />
      </main>
    </div>
  );
}
