import type { DashboardMetrics } from "@shared/types.js";
import type { ReactNode } from "react";
import { useMetrics, useRepository } from "../api/hooks.js";
import { roundMetric, computeEstimates } from "../report/estimates.js";
import { buildWbsRows } from "../report/wbs.js";
import { buildSprintRows, phaseRows } from "../report/sprints.js";

function formatForecast(metrics: DashboardMetrics): string {
  const c = metrics.forecast.completion;
  if (!c.p80Date) return "No forecast yet";
  return `P50 ${c.p50Date ?? "-"} / P80 ${c.p80Date} / P90 ${c.p90Date ?? "-"}`;
}

function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="report-kpi">
      <div className="report-kpi-label">{label}</div>
      <div className="report-kpi-value">{value}</div>
      {sub && <div className="report-kpi-sub">{sub}</div>}
    </div>
  );
}

function ReportTable({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`report-table-wrap ${className}`}><table className="report-table">{children}</table></div>;
}

export function ReportView() {
  const repository = useRepository();
  const metrics = useMetrics();
  if (repository.isLoading || metrics.isLoading) return <div className="view">Loading...</div>;
  if (repository.error || metrics.error) return <div className="view">Failed to load report data.</div>;

  const repo = repository.data!;
  const m = metrics.data!;
  const estimates = computeEstimates(repo.stories, m, repo);
  const wbsRows = buildWbsRows(repo, estimates.estimates, estimates.hoursPerPoint);
  const sprintRows = buildSprintRows(repo, m, estimates.dailyAvg, estimates.source);
  const phases = phaseRows(repo);
  const generated = m.forecast.generatedAt.slice(0, 10);
  const completedPct = m.progress.totalPoints > 0 ? Math.round((m.progress.donePoints / m.progress.totalPoints) * 100) : 0;

  return (
    <div className="view report-view">
      <div className="report-hero">
        <div>
          <div className="report-eyebrow">AutoPASS IP 2.0</div>
          <h2>WBS Report</h2>
          <p>Web version of the Excel WBS report generated from the live markdown backlog.</p>
        </div>
        <div className="report-generated">Generated {generated}</div>
      </div>

      <div className="report-kpi-grid">
        <Kpi label="Project complete" value={`${completedPct}%`} sub={`${m.progress.donePoints} / ${m.progress.totalPoints} pts`} />
        <Kpi label="Throughput" value={`${roundMetric(estimates.dailyAvg)} pts/workday`} sub={estimates.source} />
        <Kpi label="Forecast" value={m.forecast.completion.p80Date ?? "-"} sub={formatForecast(m)} />
        <Kpi label="Hours/point" value={estimates.hoursPerPoint > 0 ? `${roundMetric(estimates.hoursPerPoint)}h` : "-"} sub={`${m.forecast.remainingPoints} pts remaining`} />
      </div>

      <div className="report-main">
        <section className="report-section">
          <h3>WBS</h3>
          <ReportTable>
            <thead>
              <tr>
                <th>WBS No</th><th>ID</th><th>Title</th><th>Milestone</th><th>Period</th><th>Priority</th><th>Status</th><th>Story Pts</th><th>Est Hours</th><th>Start Date</th><th>End Date</th><th>Notes</th>
              </tr>
            </thead>
            <tbody>
              {wbsRows.map((row) => (
                <tr key={`${row.kind}-${row.wbs}-${row.id}`} className={`report-row-${row.kind} ${row.status === "DONE" ? "report-row-done" : ""} ${row.status === "IN PROGRESS" ? "report-row-active" : ""}`}>
                  <td>{row.wbs}</td><td>{row.id}</td><td>{row.title}</td><td>{row.milestone}</td><td>{row.period}</td><td>{row.priority}</td><td>{row.status}</td><td>{row.points ?? ""}</td><td>{row.estHours ?? ""}</td><td>{row.startDate ?? ""}</td><td>{row.endDate ?? ""}</td><td>{row.notes}</td>
                </tr>
              ))}
            </tbody>
          </ReportTable>
        </section>

        <div className="report-side">
          <section className="report-section report-side-phases">
            <h3>Phase & Milestone Summary</h3>
            <ReportTable>
              <thead><tr><th>Phase</th><th>Title</th><th>Period</th><th>Milestone</th><th>Epics</th><th>Stories</th><th>Total</th><th>Done</th><th>WIP</th><th>Remaining</th></tr></thead>
              <tbody>
                {phases.map((row) => <tr key={row.phase}><td>{row.phase}</td><td>{row.title}</td><td>{row.period}</td><td>{row.milestone}</td><td>{row.epics}</td><td>{row.stories}</td><td>{row.total}</td><td>{row.done}</td><td>{row.wip}</td><td>{row.remaining}</td></tr>)}
              </tbody>
            </ReportTable>
          </section>

          <section className="report-section report-side-sprints">
            <h3>Sprint Burndown & Prognosis</h3>
            <ReportTable>
              <thead><tr><th>Sprint</th><th>Start</th><th>End</th><th>Planned</th><th>Delivered</th><th>Rate</th><th>Remaining</th><th>Status</th></tr></thead>
              <tbody>
                {sprintRows.map((row) => <tr key={row.name} className={row.status.startsWith("projected") ? "report-row-projected" : ""}><td>{row.name}</td><td>{row.startDate}</td><td>{row.endDate}</td><td>{row.plannedPoints ?? ""}</td><td>{row.deliveredPoints ?? ""}</td><td>{row.rate ?? ""}</td><td>{row.remaining ?? ""}</td><td>{row.status}</td></tr>)}
              </tbody>
            </ReportTable>
          </section>
        </div>
      </div>

      <section className="report-section report-legend">
        <span className="report-legend-label">Legend</span>
        <span className="report-swatch report-row-phase" title="Top-level project phase (F1-F5)">Phase</span>
        <span className="report-swatch report-row-epic" title="Epic grouping user stories within a phase">Epic</span>
        <span className="report-swatch report-row-active" title="Story currently being developed">In Progress</span>
        <span className="report-swatch report-row-done" title="Completed and accepted story">Done</span>
        <span className="report-swatch report-row-projected" title="Future sprint projection based on daily throughput">Projected</span>
      </section>
    </div>
  );
}
