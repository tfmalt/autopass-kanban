import type { ReportWbsRow } from "@shared/generated/api.js";
import { useReport } from "../api/hooks.js";

type LeveransestiStep = {
  sequence: string;
  service: string;
  title: string;
  story: ReportWbsRow;
};

type Leveransesti = {
  id: string;
  steps: LeveransestiStep[];
  support: LeveransestiStep[];
};

export function groupLeveransestier(rows: ReportWbsRow[]): Leveransesti[] {
  const paths = new Map<string, Leveransesti>();
  for (const story of rows) {
    if (story.kind !== "story") continue;
    const match = /^(H\d+)[-.](X?\d+[A-Z]?)\s*-\s*([^-]+?)\s*-\s*(.+)$/i.exec(story.title);
    if (!match) continue;
    const [, rawId, rawSequence, service, title] = match;
    if (!rawId || !rawSequence || !service || !title) continue;
    const id = rawId.toUpperCase();
    const sequence = rawSequence.toUpperCase();
    const path = paths.get(id) ?? { id, steps: [], support: [] };
    const step = { sequence, service: service.trim(), title: title.trim(), story };
    (sequence.startsWith("X") ? path.support : path.steps).push(step);
    paths.set(id, path);
  }
  const compareSteps = (first: LeveransestiStep, second: LeveransestiStep) =>
    first.sequence.localeCompare(second.sequence, undefined, { numeric: true }) || first.story.id.localeCompare(second.story.id);
  return [...paths.values()].sort((first, second) => first.id.localeCompare(second.id, undefined, { numeric: true })).map((path) => ({
    ...path,
    steps: path.steps.sort(compareSteps),
    support: path.support.sort(compareSteps),
  }));
}

function Step({ step, pathId }: { step: LeveransestiStep; pathId: string }) {
  const state = step.story.status.toLowerCase().replaceAll(" ", "-");
  return (
    <li className={`leveransesti-step leveransesti-state-${state}`}>
      <span className="leveransesti-sequence">{pathId}-{step.sequence}</span>
      <div className="leveransesti-step-body">
        <strong>{step.service}</strong>
        <span>{step.title}</span>
        <small>{step.story.id} · {step.story.points ?? "-"} pts · {step.story.milestone}</small>
      </div>
      <span className={`leveransesti-status leveransesti-state-${state}`}>{step.story.status}</span>
    </li>
  );
}

export function LeveransestierView() {
  const query = useReport();
  if (query.isLoading) return <div className="view" role="status">Loading leveransestier...</div>;
  if (query.error) return <div className="view" role="alert">Failed to load leveransestier.</div>;
  const paths = groupLeveransestier(query.data!.wbsRows);

  return (
    <div className="view leveransestier-view">
      <header className="leveransestier-heading">
        <h2>Leveransestier</h2>
        <span>Happy Paths · {paths.length} {paths.length === 1 ? "path" : "paths"}</span>
      </header>
      {paths.length === 0 && <p>No leveransestier in this backlog yet.</p>}
      {paths.map((path) => {
        const stories = [...path.steps, ...path.support];
        const done = stories.filter((step) => step.story.status === "DONE").length;
        const dropped = stories.filter((step) => step.story.status === "DROPPED").length;
        const percent = Math.round(done / stories.length * 100);
        const isH1 = path.id === "H1";
        return (
          <section className="leveransesti" key={path.id} aria-labelledby={`leveransesti-${path.id}`}>
            <div className="leveransesti-heading">
              <h3 id={`leveransesti-${path.id}`}>{path.id}{isH1 ? " - Full price for M1 vehicle without agreement" : " - Leveransesti"}</h3>
              <span className="leveransesti-source">Live backlog</span>
            </div>
            <div className="leveransesti-summary">
              <div><strong>{percent}%</strong><span>{done} / {stories.length} stories done{dropped > 0 ? ` · ${dropped} dropped` : ""}</span></div>
              <progress max={stories.length} value={done} aria-label={`${path.id} story completion`} />
              <div><span>E2E verification</span><strong>Not verified</strong></div>
            </div>
            {isH1 && (
              <dl className="leveransesti-boundaries">
                <div><dt>Input</dt><dd>AP-3.8 TR · OBE passage · signal code 02</dd></div>
                <div><dt>Expected Output</dt><dd>AP-3.7 TIF · M1 / rate group 1 · full base price · no agreement</dd></div>
              </dl>
            )}
            <ol className="leveransesti-flow" aria-label={`${path.id} service sequence`}>
              {path.steps.map((step) => <Step key={step.story.id} step={step} pathId={path.id} />)}
            </ol>
            {path.support.length > 0 && (
              <div className="leveransesti-support">
                <h4>Supporting work</h4>
                <ul className="leveransesti-flow" aria-label={`${path.id} supporting work`}>
                  {path.support.map((step) => <Step key={step.story.id} step={step} pathId={path.id} />)}
                </ul>
              </div>
            )}
          </section>
        );
      })}
    </div>
  );
}