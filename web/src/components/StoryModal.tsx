import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { STORY_STATUSES, TASK_STATUSES, parseAssignees } from "@shared/types.js";
import type { Story, TaskStatus } from "@shared/types.js";
import { useConfig, useRepository, useTeam } from "../api/hooks.js";
import { useStory, useUpdateStory, useUpdateStoryFields, useUpdateTaskStatus } from "../api/hooks.js";

// Configure marked: GitHub-flavoured markdown, no mangling of email links.
marked.use({ gfm: true, breaks: false });

interface Props {
  story: Story;
  onClose: () => void;
  statusOptions?: StoryStatusOption[];
}

export interface StoryStatusOption {
  value: string;
  label: string;
}

const DEFAULT_STATUS_OPTIONS: StoryStatusOption[] = STORY_STATUSES.map((status) => ({
  value: status,
  label: status,
}));

function statusBadgeStyle(status: string): React.CSSProperties {
  const map: Record<string, React.CSSProperties> = {
    "in-progress": { background: "var(--accent-soft)", color: "var(--accent)" },
    "ready-for-qa": { background: "var(--green-soft)", color: "var(--green)" },
    done: { background: "var(--green-soft)", color: "var(--green)" },
    blocked: { background: "var(--red-soft)", color: "var(--red)" },
    todo: { background: "var(--surface-2)", color: "var(--text-muted)" },
  };
  return {
    fontSize: 10,
    fontWeight: 650,
    padding: "2px 8px",
    borderRadius: 10,
    whiteSpace: "nowrap" as const,
    ...(map[status] ?? map["todo"]),
  };
}

function normalizeAssigneeValue(value: string): string {
  return parseAssignees(value).join(", ");
}

function normalizeTaskStatus(status: string): TaskStatus | "" {
  const normalized = status.toLowerCase().trim().replace(/\s+/g, "-").replace(/^to-do$/, "todo");
  return TASK_STATUSES.includes(normalized as TaskStatus) ? (normalized as TaskStatus) : "";
}

function autocompleteAssigneeValue(value: string, cursor: number, options: string[]) {
  const before = value.slice(0, cursor);
  const segmentIndex = Math.max(0, before.split(",").length - 1);
  const segments = value.split(",");
  const token = segments[segmentIndex]?.trim() ?? "";
  const match = options.find((option) => option.toLowerCase().startsWith(token.toLowerCase()));
  if (!match) return null;

  const trailingSeparator = cursor === value.length && segmentIndex === segments.length - 1;
  segments[segmentIndex] = match;
  if (trailingSeparator) segments.push("");

  const nextValue = segments.map((part) => part.trim()).join(", ");
  const nextCursor = trailingSeparator
    ? nextValue.length
    : segments.slice(0, segmentIndex + 1).map((part) => part.trim()).join(", ").length;

  return { nextValue, nextCursor };
}

export function StoryModal({ story, onClose, statusOptions = DEFAULT_STATUS_OPTIONS }: Props) {
  const { data: detail, isLoading, isError } = useStory(story.id);
  const config = useConfig();
  const repo = useRepository();
  const team = useTeam();
  const updateBody = useUpdateStory();
  const updateFields = useUpdateStoryFields();
  const updateTaskStatus = useUpdateTaskStatus();
  const assigneeInputRef = useRef<HTMLInputElement | null>(null);

  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [draftAssignee, setDraftAssignee] = useState("");
  const [draftSprint, setDraftSprint] = useState("");
  const [draftStatus, setDraftStatus] = useState(story.status);
  const [draftStoryPoints, setDraftStoryPoints] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [taskSaveError, setTaskSaveError] = useState<string | null>(null);

  const currentStatus = detail?.status ?? story.status;
  const currentStoryPoints = detail?.storyPoints ?? story.storyPoints;
  const currentAssignee = detail?.assignee ?? story.assignee ?? "";
  const currentSprint = detail?.sprint ?? story.sprint ?? "";
  const currentAssigneeValue = normalizeAssigneeValue(currentAssignee);
  const currentStoryPointsValue = currentStoryPoints === null ? "" : String(currentStoryPoints);
  const currentAssigneeList = detail?.assignees ?? story.assignees;
  const currentTasks = detail?.tasks ?? story.tasks;
  const currentTaskSummary = detail?.taskSummary ?? story.taskSummary;
  const displayedStatus = editing ? draftStatus : currentStatus;
  const displayedStoryPoints = editing && draftStoryPoints.trim() !== "" ? draftStoryPoints.trim() : currentStoryPointsValue;

  // Available sprint names for the select
  const sprintOptions = useMemo(
    () => repo.data?.sprints.map((s) => s.name) ?? [],
    [repo.data],
  );

  const assigneeOptions = useMemo(() => {
    const options = new Set((team.data ?? []).map((m) => m.label));
    currentAssigneeList.forEach((assignee) => options.add(assignee));
    return [...options].sort();
  }, [team.data, currentAssigneeList]);

  const storyPointOptions = useMemo(() => {
    const options = new Set(config.data?.storyPoints.allowedValues ?? []);
    if (currentStoryPointsValue) options.add(currentStoryPointsValue);
    if (draftStoryPoints.trim()) options.add(draftStoryPoints.trim());
    return [...options];
  }, [config.data, currentStoryPointsValue, draftStoryPoints]);

  const effectiveStatusOptions = useMemo(() => {
    if (statusOptions.some((option) => option.value === currentStatus)) return statusOptions;
    return [{ value: currentStatus, label: currentStatus }, ...statusOptions];
  }, [currentStatus, statusOptions]);

  const displayedStatusLabel = effectiveStatusOptions.find((option) => option.value === displayedStatus)?.label ?? displayedStatus;

  // Render markdown → HTML (synchronous in marked v9+) and sanitize against XSS
  const renderedHtml = useMemo<string>(() => {
    if (!detail) return "";
    const result = marked.parse(detail.body);
    const html = typeof result === "string" ? result : "";
    return DOMPurify.sanitize(html);
  }, [detail]);

  const handleEdit = useCallback(() => {
    setSaveError(null);
    setDraft(detail?.body ?? "");
    setDraftAssignee(currentAssigneeValue);
    setDraftSprint(currentSprint);
    setDraftStatus(currentStatus);
    setDraftStoryPoints(currentStoryPointsValue);
    setEditing(true);
  }, [currentAssigneeValue, currentSprint, currentStatus, currentStoryPointsValue, detail]);

  const handleCancel = useCallback(() => {
    setEditing(false);
    setSaveError(null);
  }, []);

  const handleSave = useCallback(() => {
    setSaveError(null);

    const bodyChanged = detail && draft !== detail.body;
    const assigneeChanged = normalizeAssigneeValue(draftAssignee) !== currentAssigneeValue;
    const sprintChanged = draftSprint !== currentSprint && draftSprint !== "";
    const storyPointsChanged = draftStoryPoints.trim() !== currentStoryPointsValue;
    const statusChanged = draftStatus !== currentStatus;

    // Build a list of promises to run in the right order.
    // Fields are applied first so the body write lands after any metadata update.
    const ops: Array<() => Promise<void>> = [];

    if (sprintChanged || assigneeChanged || storyPointsChanged || statusChanged) {
      const fields: { assignee?: string; sprint?: string; status?: string; storyPoints?: string } = {};
      if (sprintChanged) {
        fields.sprint = draftSprint;
        fields.status = "todo";
      } else if (statusChanged) {
        fields.status = draftStatus;
      }
      if (assigneeChanged) fields.assignee = normalizeAssigneeValue(draftAssignee);
      if (storyPointsChanged) fields.storyPoints = draftStoryPoints.trim();
      ops.push(
        () =>
          new Promise<void>((resolve, reject) =>
            updateFields.mutate(
              { id: story.id, fields },
              { onSuccess: () => resolve(), onError: reject },
            ),
          ),
      );
    }

    if (bodyChanged) {
      ops.push(
        () =>
          new Promise<void>((resolve, reject) =>
            updateBody.mutate(
              { id: story.id, body: draft },
              { onSuccess: () => resolve(), onError: reject },
            ),
          ),
      );
    }

    if (ops.length === 0) {
      setEditing(false);
      return;
    }

    // Run sequentially
    ops
      .reduce((chain, op) => chain.then(op), Promise.resolve())
      .then(() => setEditing(false))
      .catch((err: unknown) =>
        setSaveError(err instanceof Error ? err.message : "Save failed"),
      );
  }, [currentAssigneeValue, currentSprint, currentStatus, currentStoryPointsValue, detail, draft, draftAssignee, draftSprint, draftStatus, draftStoryPoints, story, updateBody, updateFields]);

  const handleAssigneeKeyDown = useCallback((event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Tab") return;
    const input = event.currentTarget;
    const cursor = input.selectionStart;
    if (cursor === null || input.selectionEnd === null || cursor !== input.selectionEnd) return;

    const next = autocompleteAssigneeValue(input.value, cursor, assigneeOptions);
    if (!next) return;

    event.preventDefault();
    setDraftAssignee(next.nextValue);
    requestAnimationFrame(() => {
      input.setSelectionRange(next.nextCursor, next.nextCursor);
    });
  }, [assigneeOptions]);

  const handleTaskStatusChange = useCallback((taskId: string, status: TaskStatus) => {
    setTaskSaveError(null);
    updateTaskStatus.mutate(
      { storyId: story.id, taskId, status },
      {
        onError: (err) => setTaskSaveError(err instanceof Error ? err.message : "Task update failed"),
      },
    );
  }, [story.id, updateTaskStatus]);

  // Close on Escape (only when not editing)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !editing) onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose, editing]);

  const isSaving = updateBody.isPending || updateFields.isPending;
  const isTaskSaving = updateTaskStatus.isPending;

  return (
    <div className="overlay" onClick={editing ? undefined : onClose}>
      <div className="story-panel" onClick={(e) => e.stopPropagation()}>
        {/* ── Header ── */}
        <div className="story-panel-header">
          <div className="story-panel-header-row">
            <span className="tid" style={{ fontSize: 11 }}>{story.id}</span>
            {displayedStoryPoints !== "" && (
              <span className="pts">{displayedStoryPoints} pts</span>
            )}
            <span style={statusBadgeStyle(displayedStatus)}>{displayedStatusLabel}</span>
            <div className="story-panel-actions">
              {!editing && (
                <button
                  className="button-add"
                  style={{ fontSize: 12, padding: "4px 10px" }}
                  onClick={handleEdit}
                  disabled={isLoading || isError}
                  title={isLoading ? "Loading…" : isError ? "Could not load story" : "Edit story"}
                >
                  Edit
                </button>
              )}
              {editing && (
                <>
                  <button
                    className="button-primary"
                    style={{ fontSize: 12, padding: "4px 10px" }}
                    onClick={handleSave}
                    disabled={isSaving}
                  >
                    {isSaving ? "Saving…" : "Save"}
                  </button>
                  <button
                    className="button-add"
                    style={{ fontSize: 12, padding: "4px 10px" }}
                    onClick={handleCancel}
                    disabled={isSaving}
                  >
                    Cancel
                  </button>
                </>
              )}
              <button
                className="button-add"
                style={{ fontSize: 12, padding: "4px 10px" }}
                onClick={onClose}
                title="Close (Esc)"
                disabled={isSaving}
              >
                ✕
              </button>
            </div>
          </div>
          <h2 style={{ margin: "4px 0 6px", fontSize: 16, fontWeight: 650, lineHeight: 1.3 }}>
            {story.title}
          </h2>

          {/* Metadata — read view */}
          {!editing && (
            <div className="story-panel-meta">
              {[
                currentAssignee,
                story.epic && `Epic: ${story.epic}`,
                currentSprint && `Sprint: ${currentSprint}`,
              ]
                .filter(Boolean)
                .join(" · ")}
            </div>
          )}

          {/* Metadata — edit view */}
          {editing && (
            <div className="story-panel-fields">
              <label className="story-panel-field">
                <span className="story-panel-field-label">Assignee(s)</span>
                <input
                  ref={assigneeInputRef}
                  className="field"
                  type="text"
                  value={draftAssignee}
                  onChange={(e) => setDraftAssignee(e.target.value)}
                  placeholder="Name <email@example.com>, Name <email@example.com>"
                  list="story-panel-team-list"
                  onKeyDown={handleAssigneeKeyDown}
                />
                <datalist id="story-panel-team-list">
                  {assigneeOptions.map((member) => (
                    <option key={member} value={member}>{member}</option>
                  ))}
                </datalist>
                <div style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 4 }}>
                  Press Tab to autocomplete, and separate multiple assignees with commas.
                </div>
              </label>
              <label className="story-panel-field">
                <span className="story-panel-field-label">Status</span>
                <select
                  className="field"
                  value={draftStatus}
                  onChange={(e) => setDraftStatus(e.target.value)}
                >
                  {effectiveStatusOptions.map((status) => (
                    <option key={status.value} value={status.value}>{status.label}</option>
                  ))}
                </select>
              </label>
              <label className="story-panel-field">
                <span className="story-panel-field-label">Story points</span>
                <select
                  className="field"
                  value={draftStoryPoints}
                  onChange={(e) => setDraftStoryPoints(e.target.value)}
                >
                  {draftStoryPoints === "" && <option value="">— unset —</option>}
                  {storyPointOptions.map((points) => (
                    <option key={points} value={points}>{points}</option>
                  ))}
                </select>
              </label>
              <label className="story-panel-field">
                <span className="story-panel-field-label">Sprint</span>
                <select
                  className="field"
                  value={draftSprint}
                  onChange={(e) => {
                    const nextSprint = e.target.value;
                    setDraftSprint(nextSprint);
                    if (nextSprint !== currentSprint) setDraftStatus("todo");
                  }}
                >
                  {draftSprint === "" && <option value="">— unassigned —</option>}
                  {sprintOptions.map((name) => (
                    <option key={name} value={name}>{name}</option>
                  ))}
                </select>
              </label>
              {draftSprint !== currentSprint && draftSprint !== "" && (
                <div className="story-panel-sprint-notice">
                  Sprint change will reset status to <strong>todo</strong>
                </div>
              )}
            </div>
          )}

          {saveError && <div className="story-panel-save-error">{saveError}</div>}
        </div>

        {/* ── Body ── */}
        <div className="story-panel-content">
          {isLoading && (
            <div style={{ color: "var(--text-faint)", fontSize: 13, padding: "8px 0" }}>Loading…</div>
          )}
          {isError && (
            <div style={{ color: "var(--red)", fontSize: 13, padding: "8px 0" }}>
              Could not load story content.
            </div>
          )}
          {detail && !editing && (
            <div
              className="story-panel-markdown"
              // Rendered markdown is sanitized via DOMPurify to prevent XSS.
              // eslint-disable-next-line react/no-danger
              dangerouslySetInnerHTML={{ __html: renderedHtml }}
            />
          )}
          {detail && editing && (
            <textarea
              className="story-panel-body"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              spellCheck={false}
              autoFocus
            />
          )}
        </div>

        {/* ── Tasks ── */}
        {currentTasks.length > 0 && (
          <div className="story-panel-tasks">
            <h3 style={{ fontSize: 12, margin: "14px 0 8px", color: "var(--text-muted)" }}>
              Tasks ({currentTaskSummary.done}/{currentTaskSummary.total})
            </h3>
            {taskSaveError && <div className="story-panel-save-error">{taskSaveError}</div>}
            {currentTasks.map((task) => {
              const normalizedTaskStatus = normalizeTaskStatus(task.status);
              const isSelected = selectedTaskId === task.id;
              return (
              <div
                key={task.id}
                className={`task-card task-card--${normalizedTaskStatus || "unknown"}`}
              >
                <button
                  type="button"
                  className="task-card-button"
                  onClick={() => setSelectedTaskId(isSelected ? null : task.id)}
                  aria-expanded={isSelected}
                >
                  <span className="task-card-meta">{task.id} · {task.status}</span>
                  <span className="task-card-title">{task.title}</span>
                </button>
                {isSelected && (
                  <label className="task-status-field">
                    <span>Update status</span>
                    <select
                      className="field"
                      value={normalizedTaskStatus}
                      onChange={(e) => handleTaskStatusChange(task.id, e.target.value as TaskStatus)}
                      disabled={isTaskSaving}
                      aria-label={`Update status for ${task.id}`}
                    >
                      {normalizedTaskStatus === "" && <option value="">{task.status}</option>}
                      {TASK_STATUSES.map((status) => (
                        <option key={status} value={status}>{status}</option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
