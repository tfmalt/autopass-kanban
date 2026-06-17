import {
  Bar, BarChart, CartesianGrid, Legend, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis,
} from "recharts";
import type { BurnupPoint, DashboardMetrics } from "@shared/types.js";
import { dateToTime, daysBetween, formatDate } from "@shared/dates.js";
import { useMetrics, useRepository } from "../api/hooks.js";
import { PhaseBreakdown } from "../components/PhaseBreakdown.js";

interface ForecastRow {
  time: number;
  date: string;
  target: number | null;
  targetProjection: number | null;
  actual: number | null;
  p50: number | null;
  p80: number | null;
  p90: number | null;
}

interface ForecastModel {
  rows: ForecastRow[];
}

function roundMetric(value: number): number {
  return Number(value.toFixed(2));
}

function interpolate(points: BurnupPoint[], date: string, key: "completed" | "scope"): number {
  const first = points[0]!;
  if (date <= first.date) return first[key];

  for (let i = 1; i < points.length; i += 1) {
    const next = points[i]!;
    if (date <= next.date) {
      const prev = points[i - 1]!;
      const span = Math.max(1, daysBetween(prev.date, next.date));
      const elapsed = daysBetween(prev.date, date);
      return prev[key] + ((next[key] - prev[key]) * elapsed) / span;
    }
  }

  return points.at(-1)![key];
}

function steppedValue(points: BurnupPoint[], date: string, key: "scope"): number {
  const first = points[0]!;
  if (date <= first.date) return first[key];

  let current = first[key];
  for (let i = 1; i < points.length; i += 1) {
    const next = points[i]!;
    if (date < next.date) return current;
    current = next[key];
  }

  return current;
}

function projectedValue(
  date: string,
  endDate: string | null,
  lastDate: string,
  lastCompleted: number,
  totalPoints: number,
  rate: number,
): number | null {
  if (!endDate || rate <= 0 || date < lastDate || date > endDate) return null;
  return roundMetric(Math.min(totalPoints, lastCompleted + rate * daysBetween(lastDate, date)));
}

function projectedTargetValue(
  date: string,
  p50EndDate: string | null,
  lastDate: string,
  lastScope: number,
  totalPoints: number,
): number {
  if (!p50EndDate || date <= lastDate || totalPoints <= lastScope) return lastScope;
  if (date >= p50EndDate) return totalPoints;
  const span = Math.max(1, daysBetween(lastDate, p50EndDate));
  const elapsed = daysBetween(lastDate, date);
  return roundMetric(lastScope + ((totalPoints - lastScope) * elapsed) / span);
}

function buildForecastModel(metrics: DashboardMetrics): ForecastModel {
  const burnup = [...metrics.burnup].sort((a, b) => a.date.localeCompare(b.date));
  if (burnup.length === 0) return { rows: [] };

  const first = burnup[0]!;
  const last = burnup.at(-1)!;
  const totalPoints = metrics.progress.totalPoints;
  const lastScope = steppedValue(burnup, last.date, "scope");
  const remaining = Math.max(0, totalPoints - last.completed);
  const p50EndDate = metrics.forecast.completion.p50Date;
  const p80EndDate = metrics.forecast.completion.p80Date;
  const p90EndDate = metrics.forecast.completion.p90Date;
  const p50Rate = p50EndDate ? remaining / Math.max(1, daysBetween(last.date, p50EndDate)) : 0;
  const p80Rate = p80EndDate ? remaining / Math.max(1, daysBetween(last.date, p80EndDate)) : 0;
  const p90Rate = p90EndDate ? remaining / Math.max(1, daysBetween(last.date, p90EndDate)) : 0;
  const horizon = Math.max(
    dateToTime(last.date),
    ...[p50EndDate, p80EndDate, p90EndDate].filter((d): d is string => d !== null).map(dateToTime),
  );

  const times = new Set<number>([
    ...burnup.map((point) => dateToTime(point.date)),
    ...[p50EndDate, p80EndDate, p90EndDate].filter((d): d is string => d !== null).map(dateToTime),
  ]);
  times.add(dateToTime(first.date));
  times.add(dateToTime(last.date));
  times.add(horizon);

  const rows: ForecastRow[] = [...times].sort((a, b) => a - b).map((time) => {
    const date = formatDate(time);
    const isActual = date <= last.date;
    return {
      time,
      date,
      target: isActual ? roundMetric(steppedValue(burnup, date, "scope")) : null,
      targetProjection: date >= last.date
        ? projectedTargetValue(date, p50EndDate, last.date, lastScope, totalPoints)
        : null,
      actual: isActual ? roundMetric(interpolate(burnup, date, "completed")) : null,
      p50: projectedValue(date, p50EndDate, last.date, last.completed, totalPoints, p50Rate),
      p80: projectedValue(date, p80EndDate, last.date, last.completed, totalPoints, p80Rate),
      p90: projectedValue(date, p90EndDate, last.date, last.completed, totalPoints, p90Rate),
    };
  });

  return { rows };
}

function formatPercent(points: number, totalPoints: number): string {
  if (totalPoints <= 0) return "0%";
  return `${Math.round((points / totalPoints) * 100)}%`;
}

function formatTooltipValue(value: unknown, totalPoints: number): string {
  const points = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(points)) return "-";
  return `${roundMetric(points)} pts / ${formatPercent(points, totalPoints)}`;
}

function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="card" style={{ cursor: "default" }}>
      <div style={{ fontSize: 10, textTransform: "uppercase", color: "var(--text-faint)" }}>{label}</div>
      <div style={{ fontSize: 22, fontWeight: 700 }}>{value}</div>
      {sub && <div style={{ fontSize: 10, color: "var(--text-muted)" }}>{sub}</div>}
    </div>
  );
}

export function DashboardView() {
  const metrics = useMetrics();
  const repository = useRepository();
  if (metrics.isLoading) return <div className="view">Loading...</div>;
  if (metrics.error) return <div className="view">Failed to load metrics.</div>;
  const m = metrics.data!;
  const pct = m.progress.totalPoints === 0 ? 0 : Math.round((m.progress.donePoints / m.progress.totalPoints) * 100);
  const lastLeadTime = m.leadTime.at(-1);
  const forecast = buildForecastModel(m);
  const forecastRange = m.forecast.completion.p50Date && m.forecast.completion.p90Date
    ? `P50 ${m.forecast.completion.p50Date} / P80 ${m.forecast.completion.p80Date ?? "-"} / P90 ${m.forecast.completion.p90Date}`
    : "-";
  const yMax = Math.max(1, m.progress.totalPoints);

  return (
    <div className="view">
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10, marginBottom: 14 }}>
        <Kpi label="Project complete" value={`${pct}%`} sub={`${m.progress.donePoints} / ${m.progress.totalPoints} pts`} />
        <Kpi label="Throughput" value={`${roundMetric(m.forecast.throughput.average)} pts/day`} sub={`${m.forecast.confidence} confidence`} />
        <Kpi label="Projected end date" value={m.forecast.completion.p80Date ?? "-"} sub={`${m.forecast.remainingPoints} pts left`} />
        <Kpi label="Lead time (last)" value={lastLeadTime ? `${lastLeadTime.days} d` : "-"} />
      </div>

      <div className="card" style={{ cursor: "default" }}>
        <h4 style={{ marginTop: 0, marginBottom: 4 }}>Completion forecast</h4>
        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
          Linear points and completion percentage by date. Canonical forecast: {forecastRange}
        </div>
        <ResponsiveContainer width="100%" aspect={3}>
          <LineChart data={forecast.rows}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
            <XAxis
              dataKey="time"
              type="number"
              scale="time"
              domain={["dataMin", "dataMax"]}
              tickFormatter={(value) => formatDate(Number(value))}
              fontSize={10}
            />
            <YAxis yAxisId="points" domain={[0, yMax]} fontSize={10} tickFormatter={(value) => `${value} pts`} />
            <YAxis
              yAxisId="percent"
              orientation="right"
              domain={[0, yMax]}
              fontSize={10}
              tickFormatter={(value) => formatPercent(Number(value), m.progress.totalPoints)}
            />
            <Tooltip
              labelFormatter={(value) => formatDate(Number(value))}
              formatter={(value, name) => [formatTooltipValue(value, m.progress.totalPoints), name]}
            />
            <Legend verticalAlign="top" height={24} />
            <Line yAxisId="points" name="Target scope" type="stepAfter" dataKey="target" stroke="var(--amber)" dot={false} strokeWidth={2} />
            <Line yAxisId="points" name="Target projection" type="linear" dataKey="targetProjection" stroke="var(--amber)" strokeDasharray="6 4" dot={false} strokeWidth={2} />
            <Line yAxisId="points" name="Completed" type="linear" dataKey="actual" stroke="var(--green)" dot={false} strokeWidth={3} />
            <Line yAxisId="points" name="P50 forecast" type="linear" dataKey="p50" stroke="var(--accent-2)" strokeDasharray="4 4" dot={false} strokeWidth={2} />
            <Line yAxisId="points" name="P80 forecast" type="linear" dataKey="p80" stroke="var(--accent)" strokeDasharray="4 4" dot={false} strokeWidth={2} />
            <Line yAxisId="points" name="P90 forecast" type="linear" dataKey="p90" stroke="var(--chart-muted)" strokeDasharray="4 4" dot={false} strokeWidth={2} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginTop: 12 }}>
        <div className="card" style={{ cursor: "default" }}>
          <h4 style={{ marginTop: 0 }}>Sprint burndown</h4>
          <ResponsiveContainer width="100%" aspect={2}>
            <LineChart data={m.burndown}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="date" fontSize={10} /><YAxis fontSize={10} /><Tooltip />
              <Line type="monotone" dataKey="remaining" stroke="var(--accent)" strokeWidth={2} dot={false} />
              <Line type="monotone" dataKey="ideal" stroke="var(--chart-muted)" strokeDasharray="4 4" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
        <div className="card" style={{ cursor: "default" }}>
          <h4 style={{ marginTop: 0 }}>Velocity / throughput</h4>
          <ResponsiveContainer width="100%" aspect={2}>
            <BarChart data={m.velocity}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="sprint" fontSize={10} /><YAxis fontSize={10} /><Tooltip />
              <Bar dataKey="points" fill="var(--accent)" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div style={{ marginTop: 12 }}>
        <PhaseBreakdown phases={m.progress.phases} epics={repository.data?.epics ?? []} />
      </div>
    </div>
  );
}
