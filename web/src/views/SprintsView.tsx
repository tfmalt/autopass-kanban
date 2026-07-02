import { useMemo, useState } from "react";
import DOMPurify from "dompurify";
import { marked } from "marked";
import type { Sprint } from "@shared/types.js";
import { SPRINT_STATUSES } from "@shared/types.js";
import { useCreateSprint, useRepository, useUpdateSprint } from "../api/hooks.js";

function visibleScopePoints(stories: Sprint["storiesByStatus"][keyof Sprint["storiesByStatus"]]): number {
  return stories.reduce((sum, story) => sum + (story.status === "dropped" ? 0 : (story.storyPoints ?? 0)), 0);
}

function completedPoints(stories: Sprint["storiesByStatus"][keyof Sprint["storiesByStatus"]]): number {
  return stories.reduce((sum, story) => sum + (story.status === "done" ? (story.storyPoints ?? 0) : 0), 0);
}

marked.use({ gfm: true, breaks: false });

interface SprintFormState {
  headline: string;
  goal: string;
  start: string;
  end: string;
  status: string;
  wipLimit: string;
}

function toSprintFormState(sprint: Sprint): SprintFormState {
  return {
    headline: sprint.headline ?? "",
    goal: sprint.goal ?? "",
    start: sprint.startDate ?? "",
    end: sprint.endDate ?? "",
    status: sprint.status ?? "planned",
    wipLimit: sprint.wipLimit === null ? "" : String(sprint.wipLimit),
  };
}

export function SprintsView() {
  const repo = useRepository();
  const create = useCreateSprint();
  const update = useUpdateSprint();
  const [showModal, setShowModal] = useState(false);
  const [selected, setSelected] = useState<Sprint | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [headline, setHeadline] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [editForm, setEditForm] = useState<SprintFormState | null>(null);

  const renderGoalMarkdown = (goal: string | null | undefined) => {
    const markdown = goal?.trim();
    if (!markdown) return null;
    const result = marked.parse(markdown);
    const html = typeof result === "string" ? result : "";
    return DOMPurify.sanitize(html);
  };

  const selectedGoalHtml = useMemo(() => renderGoalMarkdown(selected?.goal), [selected?.goal]);

  if (repo.isLoading) return <div className="view">Loading...</div>;

  const submit = () => {
    create.mutate(
      { headline, start: start || undefined, end: end || undefined },
      { onSuccess: () => setShowModal(false) },
    );
  };

  const openSprint = (sprint: Sprint) => {
    setSelected(sprint);
    setIsEditing(false);
    setEditForm(null);
  };

  const closeSprint = () => {
    setSelected(null);
    setIsEditing(false);
    setEditForm(null);
  };

  const startEdit = () => {
    if (!selected) return;
    setEditForm(toSprintFormState(selected));
    setIsEditing(true);
  };

  const cancelEdit = () => {
    setIsEditing(false);
    setEditForm(null);
  };

  const submitEdit = () => {
    if (!selected || !editForm) return;
    update.mutate(
      {
        name: selected.name,
        headline: editForm.headline,
        goal: editForm.goal,
        start: editForm.start,
        end: editForm.end,
        status: editForm.status,
        wipLimit: editForm.wipLimit === "" ? null : Number(editForm.wipLimit),
      },
      { onSuccess: (response) => {
        setSelected({
          ...selected,
          name: response.data.name,
          headline: response.data.headline,
          goal: editForm.goal,
          startDate: editForm.start,
          endDate: editForm.end,
          status: editForm.status,
          wipLimit: editForm.wipLimit === "" ? null : Number(editForm.wipLimit),
        });
        setIsEditing(false);
        setEditForm(null);
      } },
    );
  };

  return (
    <div className="view">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ marginTop: 0 }}>Sprints</h2>
        <button onClick={() => setShowModal(true)} className="button-primary">
          + Create sprint
        </button>
      </div>

      {repo.data!.sprints.map((s) => {
        const done = completedPoints(s.storiesByStatus.done);
        const total = Object.values(s.storiesByStatus).reduce((sum, bucket) => sum + visibleScopePoints(bucket), 0);
        return (
          <div
            key={s.name}
            className="card"
            role="button"
            tabIndex={0}
            aria-label={`Open sprint ${s.name}`}
            onClick={() => openSprint(s)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                openSprint(s);
              }
            }}
            style={{ cursor: "pointer", display: "grid", gridTemplateColumns: "200px 1fr 160px", gap: 14, alignItems: "center" }}
          >
            <div><b>{s.name}</b><div style={{ fontSize: 10, color: "var(--text-faint)" }}>{s.status}</div></div>
             <div style={{ fontSize: 12, color: "var(--text-muted)", whiteSpace: "pre-wrap" }}>{s.goal ?? ""}</div>
            <div style={{ textAlign: "right", fontSize: 12, color: "var(--text-muted)" }}>
              {s.startDate} → {s.endDate}<br />{done}/{total} pts done
            </div>
          </div>
        );
      })}

      {showModal && (
        <div onClick={() => setShowModal(false)} className="overlay modal-wrap">
          <div onClick={(e) => e.stopPropagation()} className="modal">
            <h3 style={{ marginTop: 0 }}>Create sprint</h3>
            <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
              Headline slug
              <input aria-label="headline" value={headline} onChange={(e) => setHeadline(e.target.value)} className="field" style={{ display: "block", marginTop: 3 }} />
            </label>
            <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
              Start date
              <input aria-label="start date" type="date" value={start} onChange={(e) => setStart(e.target.value)} className="field" style={{ display: "block", marginTop: 3 }} />
            </label>
            <label style={{ display: "block", fontSize: 12, marginBottom: 12 }}>
              End date
              <input aria-label="end date" type="date" value={end} onChange={(e) => setEnd(e.target.value)} className="field" style={{ display: "block", marginTop: 3 }} />
            </label>
            {create.error && <div style={{ color: "var(--red)", marginBottom: 8 }}>Create failed: {String(create.error)}</div>}
            <button onClick={submit} disabled={!headline} className="button-primary">
              Create
            </button>
          </div>
        </div>
      )}

      {selected && (
        <div onClick={closeSprint} className="overlay">
          <div onClick={(e) => e.stopPropagation()} className="drawer">
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 12 }}>
              <div>
                <div className="tid">{selected.id} · {selected.status ?? "unknown"}</div>
                <h2 style={{ marginTop: 4 }}>{selected.name}</h2>
              </div>
              {!isEditing && (
                <button onClick={startEdit} className="button-secondary">
                  Edit
                </button>
              )}
            </div>
            <p style={{ color: "var(--text-muted)", fontSize: 13, marginTop: 0 }}>
              {selected.startDate ?? "No start date"} → {selected.endDate ?? "No end date"} · {selected.headline}
            </p>

            {!isEditing && (
              <>
                <h3 style={{ fontSize: 13 }}>Sprint goal</h3>
                {selectedGoalHtml ? (
                  <div
                    className="sprint-goal-markdown"
                    style={{ color: "var(--text-muted)", fontSize: 13 }}
                    // Rendered markdown is sanitized via DOMPurify to preserve safe formatting.
                    // eslint-disable-next-line react/no-danger
                    dangerouslySetInnerHTML={{ __html: selectedGoalHtml }}
                  />
                ) : (
                  <p style={{ color: "var(--text-muted)", fontSize: 13 }}>No sprint goal set.</p>
                )}
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginTop: 16 }}>
                  <div style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 8 }}>
                    <div style={{ fontSize: 11, color: "var(--text-faint)" }}>WIP limit</div>
                    <div style={{ fontSize: 13 }}>{selected.wipLimit ?? "Not set"}</div>
                  </div>
                  <div style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 8 }}>
                    <div style={{ fontSize: 11, color: "var(--text-faint)" }}>Stories</div>
                    <div style={{ fontSize: 13 }}>{Object.values(selected.storiesByStatus).flat().length}</div>
                  </div>
                </div>
              </>
            )}

            {isEditing && editForm && (
              <>
                <h3 style={{ fontSize: 13 }}>Edit sprint</h3>
                <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
                  Sprint headline
                  <input aria-label="edit sprint headline" value={editForm.headline} onChange={(e) => setEditForm({ ...editForm, headline: e.target.value })} className="field" style={{ display: "block", marginTop: 3 }} />
                </label>
                <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
                  Sprint goal
                  <textarea aria-label="edit sprint goal" value={editForm.goal} onChange={(e) => setEditForm({ ...editForm, goal: e.target.value })} className="field sprint-goal-field" rows={4} style={{ display: "block", marginTop: 3 }} />
                </label>
                <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
                  Start date
                  <input aria-label="edit start date" type="date" value={editForm.start} onChange={(e) => setEditForm({ ...editForm, start: e.target.value })} className="field" style={{ display: "block", marginTop: 3 }} />
                </label>
                <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
                  End date
                  <input aria-label="edit end date" type="date" value={editForm.end} onChange={(e) => setEditForm({ ...editForm, end: e.target.value })} className="field" style={{ display: "block", marginTop: 3 }} />
                </label>
                <label style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
                  Status
                  <select aria-label="edit status" value={editForm.status} onChange={(e) => setEditForm({ ...editForm, status: e.target.value })} className="field" style={{ display: "block", marginTop: 3 }}>
                    {SPRINT_STATUSES.map((status) => (
                      <option key={status} value={status}>{status}</option>
                    ))}
                  </select>
                </label>
                <label style={{ display: "block", fontSize: 12, marginBottom: 12 }}>
                  WIP limit
                  <input aria-label="edit wip limit" type="number" min="0" value={editForm.wipLimit} onChange={(e) => setEditForm({ ...editForm, wipLimit: e.target.value })} className="field" style={{ display: "block", marginTop: 3 }} />
                </label>
                {update.error && <div style={{ color: "var(--red)", marginBottom: 8 }}>Update failed: {String(update.error)}</div>}
                <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                  <button onClick={cancelEdit} className="button-secondary">
                    Cancel
                  </button>
                  <button onClick={submitEdit} disabled={!editForm.headline.trim() || !editForm.start || !editForm.end || !editForm.status} className="button-primary">
                    Save
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
