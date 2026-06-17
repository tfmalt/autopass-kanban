import { useMemo, useRef, useState, type ReactNode } from "react";
import {
  DndContext,
  DragOverlay,
  KeyboardSensor,
  PointerSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
  type DraggableAttributes,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { STORY_STATUSES, type Epic, type Story } from "@shared/types.js";
import { byPriorityThenId, useEpic, usePlanStory, useReorderEpics, useReorderStories, useRepository, useUnplanStory } from "../api/hooks.js";
import { StoryModal } from "../components/StoryModal.js";

const SPRINT_DROP_ID = "backlog-target-sprint";
const BACKLOG_DROP_ID = "backlog-source";
const NO_EPIC_GROUP_ID = "(no epic)";

function toTransformStyle(transform: { x: number; y: number } | null, transition?: string, opacity?: number) {
  return {
    transform: transform ? `translate(${transform.x}px, ${transform.y}px)` : undefined,
    transition,
    opacity,
  };
}

function BacklogDropZone({ children, disabled }: { children: ReactNode; disabled: boolean }) {
  const { setNodeRef, isOver } = useDroppable({ id: BACKLOG_DROP_ID, disabled });
  return (
    <section ref={setNodeRef} className={`backlog-column${isOver ? " is-over" : ""}`}>
      {children}
    </section>
  );
}

function BacklogStoryContent({ story }: { story: Story }) {
  return (
    <>
      <div style={{ flex: 1 }}>
        <div className="tid">{story.id}</div>
        <div className="ttl" style={{ margin: 0 }}>{story.title}</div>
      </div>
      <span className="pts">{story.storyPoints ?? "-"}</span>
    </>
  );
}

function EpicChevron({ expanded }: { expanded: boolean }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 12 12"
      width="12"
      height="12"
      style={{
        flex: "0 0 auto",
        transform: expanded ? "rotate(90deg)" : "rotate(0deg)",
        transition: "transform 120ms ease",
      }}
    >
      <path d="M4 2.5 8 6 4 9.5" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

interface EpicPreviewSection {
  heading: string;
  markdown: string;
}

marked.use({ gfm: true, breaks: false });

function parseEpicPreview(body: string): EpicPreviewSection[] {
  const sections: EpicPreviewSection[] = [];
  const lines = body.split("\n");
  let current: EpicPreviewSection | null = null;
  let contentLines: string[] = [];

  const flushSection = () => {
    if (!current) return;
    const markdown = contentLines.join("\n").trim();
    if (markdown) sections.push({ ...current, markdown });
    contentLines = [];
  };

  for (const line of lines) {
    if (line.startsWith("## ")) {
      flushSection();
      if (sections.length === 2) break;
      current = { heading: line.replace(/^##\s+/, "").trim(), markdown: "" };
      continue;
    }
    if (!current) continue;

    const trimmed = line.trim();
    if (trimmed === "---") continue;
    contentLines.push(line);
  }

  flushSection();
  return sections.slice(0, 2);
}

function EpicContext({ epicId }: { epicId: string }) {
  const epic = useEpic(epicId);
  const preview = useMemo(() => parseEpicPreview(epic.data?.body ?? ""), [epic.data?.body]);
  const renderedSections = useMemo(() => preview.map((section) => {
    const result = marked.parse(section.markdown);
    const html = typeof result === "string" ? result : "";
    return {
      ...section,
      html: DOMPurify.sanitize(html),
    };
  }), [preview]);

  if (epic.isLoading) {
    return <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 8 }}>Loading epic context...</div>;
  }

  if (epic.isError) {
    return <div style={{ fontSize: 12, color: "var(--red)", marginBottom: 8 }}>Could not load epic context.</div>;
  }

  if (renderedSections.length === 0) {
    return <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 8 }}>No epic context available.</div>;
  }

  return (
    <div style={{ marginBottom: 10, padding: 12, background: "var(--surface-2)", borderRadius: "var(--radius)", border: "1px solid var(--border)" }}>
      {renderedSections.map((section) => (
        <section key={section.heading} style={{ marginBottom: 10 }}>
          <div style={{ fontSize: 12, fontWeight: 650, marginBottom: 6 }}>{section.heading}</div>
          <div
            style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5 }}
            // Rendered markdown is sanitized via DOMPurify to preserve safe formatting.
            // eslint-disable-next-line react/no-danger
            dangerouslySetInnerHTML={{ __html: section.html }}
          />
        </section>
      ))}
    </div>
  );
}

function BacklogStoryCardBody({
  story,
  disabled,
  onPlan,
  onOpen,
  dragAttributes,
  dragListeners,
  setNodeRef,
  style,
}: {
  story: Story;
  disabled: boolean;
  onPlan: () => void;
  onOpen: (story: Story) => void;
  dragAttributes: DraggableAttributes;
  dragListeners: ReturnType<typeof useDraggable>["listeners"];
  setNodeRef: (element: HTMLElement | null) => void;
  style: { transform?: string; transition?: string; opacity?: number };
}) {
  return (
    <div
      ref={setNodeRef}
      className="card backlog-story-card"
      style={style}
      data-testid={`story-card-${story.id}`}
      {...dragListeners}
      {...dragAttributes}
      onClick={() => onOpen(story)}
    >
      <button
        aria-label={`add ${story.id}`}
        onClick={(event) => {
          event.stopPropagation();
          onPlan();
        }}
        onPointerDown={(event) => event.stopPropagation()}
        disabled={disabled}
        className="button-add"
      >
        +
      </button>
      <BacklogStoryContent story={story} />
    </div>
  );
}

function SortableBacklogStoryCard({ story, disabled, onPlan, onOpen, sourceContext }: { story: Story; disabled: boolean; onPlan: () => void; onOpen: (story: Story) => void; sourceContext: string }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: story.id,
    data: { type: "story", sourceContext },
  });

  return (
    <BacklogStoryCardBody
      story={story}
      disabled={disabled}
      onPlan={onPlan}
      onOpen={onOpen}
      dragAttributes={attributes}
      dragListeners={listeners}
      setNodeRef={setNodeRef}
      style={toTransformStyle(transform, transition, isDragging ? 0.55 : 1)}
    />
  );
}

function DraggableBacklogStoryCard({ story, disabled, onPlan, onOpen, sourceContext }: { story: Story; disabled: boolean; onPlan: () => void; onOpen: (story: Story) => void; sourceContext: string }) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: story.id,
    data: { type: "story", sourceContext },
    disabled,
  });

  return (
    <BacklogStoryCardBody
      story={story}
      disabled={disabled}
      onPlan={onPlan}
      onOpen={onOpen}
      dragAttributes={attributes}
      dragListeners={listeners}
      setNodeRef={setNodeRef}
      style={toTransformStyle(transform, undefined, isDragging ? 0.55 : 1)}
    />
  );
}

function BacklogStoryCard(props: { story: Story; disabled: boolean; onPlan: () => void; onOpen: (story: Story) => void; sourceContext: string; sortable: boolean }) {
  if (props.sortable) {
    return <SortableBacklogStoryCard {...props} />;
  }
  return <DraggableBacklogStoryCard {...props} />;
}

function SprintStoryCard({ story, disabled, onRemove, onOpen }: { story: Story; disabled: boolean; onRemove: () => void; onOpen: (story: Story) => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: story.id,
    data: { type: "story", sourceContext: "sprint" },
    disabled,
  });

  return (
    <div
      ref={setNodeRef}
      className="card backlog-story-card"
      style={toTransformStyle(transform, transition, isDragging ? 0.55 : 1)}
      data-testid={`story-card-${story.id}`}
      {...listeners}
      {...attributes}
      onClick={() => onOpen(story)}
    >
      <BacklogStoryContent story={story} />
      <button
        aria-label={`remove ${story.id}`}
        onClick={(event) => {
          event.stopPropagation();
          onRemove();
        }}
        onPointerDown={(event) => event.stopPropagation()}
        disabled={disabled}
        className="button-add"
      >
        Remove
      </button>
    </div>
  );
}

function SprintDropZone({ stories, targetSprint, disabled, onRemove, onOpen }: { stories: Story[]; targetSprint: string; disabled: boolean; onRemove: (storyId: string) => void; onOpen: (story: Story) => void }) {
  const { setNodeRef, isOver } = useDroppable({ id: SPRINT_DROP_ID, disabled });
  const points = stories.reduce((sum, story) => sum + (story.storyPoints ?? 0), 0);

  return (
    <div ref={setNodeRef} aria-label="current sprint drop target" className={`sprint-dropzone${isOver ? " is-over" : ""}`}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "baseline" }}>
        <h3 style={{ margin: 0 }}>User Stories for Current Sprint</h3>
        <span className="pts">{stories.length} · {points} pts</span>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>{targetSprint || "No sprint selected"}</div>
      <div style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 10 }}>
        Drag backlog stories here to plan them into the selected sprint.
      </div>
      <div style={{ marginTop: 12 }}>
        {stories.length === 0 ? (
          <div style={{ fontSize: 12, color: "var(--text-muted)" }}>No user stories planned for this sprint yet.</div>
        ) : (
          <SortableContext items={stories.map((story) => story.id)} strategy={verticalListSortingStrategy}>
            {stories.map((story) => (
              <SprintStoryCard key={story.id} story={story} disabled={disabled} onRemove={() => onRemove(story.id)} onOpen={onOpen} />
            ))}
          </SortableContext>
        )}
      </div>
    </div>
  );
}

function EpicDragOverlay({ epic }: { epic: Epic }) {
  return (
    <div className="card card--overlay" data-testid="backlog-drag-overlay" style={{ marginBottom: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11, textTransform: "uppercase", color: "var(--text-muted)" }}>
        <span className="drag-handle" aria-hidden="true">::</span>
        <span style={{ fontWeight: 700, color: "var(--text)" }}>{epic.id}</span>
        <span style={{ color: "var(--text-muted)" }}>{epic.title}</span>
      </div>
    </div>
  );
}

function SortableEpicSection({
  epic,
  stories,
  collapsed,
  backlogReorderDisabled,
  targetSprint,
  planPending,
  onToggle,
  onPlan,
  onOpen,
}: {
  epic: Epic;
  stories: Story[];
  collapsed: boolean;
  backlogReorderDisabled: boolean;
  targetSprint: string;
  planPending: boolean;
  onToggle: () => void;
  onPlan: (storyId: string) => void;
  onOpen: (story: Story) => void;
}) {
  const { attributes, listeners, setActivatorNodeRef, setNodeRef, transform, transition, isDragging } = useSortable({
    id: epic.id,
    data: { type: "epic" },
    disabled: backlogReorderDisabled,
  });

  return (
    <div
      ref={setNodeRef}
      style={{
        ...toTransformStyle(transform, transition, isDragging ? 0.75 : 1),
        marginBottom: 12,
      }}
      data-testid={`epic-section-${epic.id}`}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
        <span
          ref={setActivatorNodeRef}
          className="drag-handle"
          aria-label={`reorder epic ${epic.id}`}
          onClick={(event) => event.stopPropagation()}
          {...listeners}
          {...attributes}
        >
          ::
        </span>
        <button
          type="button"
          onClick={onToggle}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            fontSize: 11,
            textTransform: "uppercase",
            color: "var(--text-muted)",
            border: 0,
            background: "none",
            padding: 0,
            cursor: "pointer",
          }}
          aria-expanded={!collapsed}
        >
          <EpicChevron expanded={!collapsed} />
          <span style={{ fontWeight: 700, color: "var(--text)" }}>{epic.id}</span>
          <span style={{ color: "var(--text-muted)" }}>{epic.title}</span>
          {collapsed && <span className="epic-story-count">({stories.length} stories)</span>}
        </button>
      </div>
      {!collapsed && (
        <>
          <EpicContext epicId={epic.id} />
          <SortableContext items={stories.map((story) => story.id)} strategy={verticalListSortingStrategy}>
            {stories.map((story) => (
              <BacklogStoryCard
                key={story.id}
                story={story}
                disabled={!targetSprint || planPending}
                onPlan={() => onPlan(story.id)}
                onOpen={onOpen}
                sourceContext={epic.id}
                sortable={!backlogReorderDisabled}
              />
            ))}
          </SortableContext>
        </>
      )}
    </div>
  );
}

export function BacklogView() {
  const repo = useRepository();
  const plan = usePlanStory();
  const unplan = useUnplanStory();
  const reorderStories = useReorderStories();
  const reorderEpics = useReorderEpics();
  const [sprint, setSprint] = useState<string>("");
  const [search, setSearch] = useState("");
  const [open, setOpen] = useState<Story | null>(null);
  const [collapsedEpics, setCollapsedEpics] = useState<Set<string>>(new Set());
  const [activeId, setActiveId] = useState<string | null>(null);
  const dragActivatedRef = useRef(false);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const backlogReorderDisabled = search.trim() !== "";
  const sprints = repo.data?.sprints ?? [];
  const targetSprint = sprint || sprints.find((candidate) => candidate.status === "planned")?.name || sprints.at(-1)?.name || "";
  const visibleSprint = sprints.find((candidate) => candidate.name === targetSprint);

  const unplanned = useMemo(() => {
    if (!repo.data) return [];
    return repo.data.stories.filter(
      (story) => !story.sprint && story.status !== "done" && (search === "" || `${story.id} ${story.title}`.toLowerCase().includes(search.toLowerCase())),
    );
  }, [repo.data, search]);

  const epicOrder = useMemo(() => {
    return byPriorityThenId(repo.data?.epics ?? []);
  }, [repo.data?.epics]);

  const storiesByEpic = useMemo(() => {
    const map = new Map<string, Story[]>();
    for (const story of unplanned) {
      const key = story.epic ?? NO_EPIC_GROUP_ID;
      map.set(key, [...(map.get(key) ?? []), story]);
    }
    for (const [key, stories] of map.entries()) {
      map.set(key, byPriorityThenId(stories));
    }
    return map;
  }, [unplanned]);

  const noEpicStories = storiesByEpic.get(NO_EPIC_GROUP_ID) ?? [];

  const targetStories = useMemo(() => {
    if (!visibleSprint) return [];
    return byPriorityThenId(STORY_STATUSES.flatMap((status) => visibleSprint.storiesByStatus[status] ?? []));
  }, [visibleSprint]);

  const activeStory = useMemo(() => repo.data?.stories.find((story) => story.id === activeId) ?? null, [activeId, repo.data?.stories]);
  const activeEpic = useMemo(() => repo.data?.epics.find((epic) => epic.id === activeId) ?? null, [activeId, repo.data?.epics]);

  const planStory = (storyId: string) => {
    if (!targetSprint) return;
    plan.mutate({ id: storyId, sprint: targetSprint });
  };

  const unplanStory = (storyId: string) => {
    unplan.mutate({ id: storyId });
  };

  const toggleEpic = (epicId: string) => {
    setCollapsedEpics((current) => {
      const next = new Set(current);
      if (next.has(epicId)) next.delete(epicId);
      else next.add(epicId);
      return next;
    });
  };

  const handleOpen = (story: Story) => {
    if (dragActivatedRef.current) {
      dragActivatedRef.current = false;
      return;
    }
    setOpen(story);
  };

  const onDragStart = (event: DragStartEvent) => {
    dragActivatedRef.current = true;
    setActiveId(String(event.active.id));
  };

  const onDragCancel = () => {
    setActiveId(null);
    dragActivatedRef.current = false;
  };

  const onDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveId(null);
    if (!over || !repo.data) return;

    const activeType = active.data.current?.type;
    const activeContext = active.data.current?.sourceContext;
    const overContext = over.data.current?.sourceContext ?? over.id;

    if (activeType === "epic" && !backlogReorderDisabled) {
      const oldOrder = epicOrder.map((epic) => epic.id);
      const oldIndex = oldOrder.indexOf(String(active.id));
      const newIndex = oldOrder.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderEpics.mutate({
          orderedIds: arrayMove(oldOrder, oldIndex, newIndex),
          movedId: String(active.id),
          items: epicOrder,
        });
      }
      return;
    }

    if (activeType === "story" && !backlogReorderDisabled && activeContext !== "sprint" && activeContext === overContext) {
      const stories = storiesByEpic.get(String(activeContext)) ?? [];
      const orderedIds = stories.map((story) => story.id);
      const oldIndex = orderedIds.indexOf(String(active.id));
      const newIndex = orderedIds.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderStories.mutate({
          orderedIds: arrayMove(orderedIds, oldIndex, newIndex),
          movedId: String(active.id),
          items: stories,
        });
      }
      return;
    }

    if (activeType === "story" && activeContext === "sprint" && overContext === "sprint") {
      const orderedIds = targetStories.map((story) => story.id);
      const oldIndex = orderedIds.indexOf(String(active.id));
      const newIndex = orderedIds.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderStories.mutate({
          orderedIds: arrayMove(orderedIds, oldIndex, newIndex),
          movedId: String(active.id),
          items: targetStories,
        });
      }
      return;
    }

    const storyId = String(active.id);
    const story = repo.data.stories.find((candidate) => candidate.id === storyId);
    if (!story) return;

    if ((over.id === SPRINT_DROP_ID || overContext === "sprint") && activeContext !== "sprint" && story.sprint !== targetSprint) {
      planStory(storyId);
      return;
    }

    if ((over.id === BACKLOG_DROP_ID || activeContext === "sprint") && story.sprint && (over.id === BACKLOG_DROP_ID || overContext !== "sprint")) {
      unplanStory(storyId);
    }
  };

  if (repo.isLoading) return <div className="view">Loading...</div>;
  if (repo.error) return <div className="view">Failed to load: {String(repo.error)}</div>;

  return (
    <DndContext sensors={sensors} onDragStart={onDragStart} onDragEnd={onDragEnd} onDragCancel={onDragCancel}>
      <div className="view backlog-planning-grid">
        <BacklogDropZone disabled={unplan.isPending}>
          <h2 style={{ marginTop: 0 }}>Backlog</h2>
          <input placeholder="Search stories..." value={search} onChange={(event) => setSearch(event.target.value)} className="field" style={{ marginBottom: 10 }} />
          {backlogReorderDisabled && (
            <p style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 0, marginBottom: 10 }}>
              Priority reordering is disabled while filtering. Planning and story details are still available.
            </p>
          )}
          <SortableContext items={epicOrder.map((epic) => epic.id)} strategy={verticalListSortingStrategy}>
            {epicOrder.map((epic) => (
              <SortableEpicSection
                key={epic.id}
                epic={epic}
                stories={storiesByEpic.get(epic.id) ?? []}
                collapsed={collapsedEpics.has(epic.id)}
                backlogReorderDisabled={backlogReorderDisabled}
                targetSprint={targetSprint}
                planPending={plan.isPending}
                onToggle={() => toggleEpic(epic.id)}
                onPlan={planStory}
                onOpen={handleOpen}
              />
            ))}
          </SortableContext>

          {noEpicStories.length > 0 && (
            <div style={{ marginBottom: 12 }} data-testid="epic-section-no-epic">
              <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11, textTransform: "uppercase", color: "var(--text-muted)", marginBottom: 6 }}>
                <span style={{ fontWeight: 700, color: "var(--text)" }}>{NO_EPIC_GROUP_ID}</span>
              </div>
              {noEpicStories.map((story) => (
                <BacklogStoryCard
                  key={story.id}
                  story={story}
                  disabled={!targetSprint || plan.isPending}
                  onPlan={() => planStory(story.id)}
                  onOpen={handleOpen}
                  sourceContext={NO_EPIC_GROUP_ID}
                  sortable={false}
                />
              ))}
            </div>
          )}
        </BacklogDropZone>

        <aside style={{ background: "var(--surface-2)", borderRadius: "var(--radius)", padding: 12 }}>
          <h2 style={{ marginTop: 0 }}>Planning</h2>
          <select value={targetSprint} onChange={(event) => setSprint(event.target.value)} className="field" aria-label="target sprint">
            {sprints.map((candidate) => (
              <option key={candidate.name} value={candidate.name}>{candidate.name} ({candidate.status})</option>
            ))}
          </select>
          {plan.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Plan failed: {String(plan.error)}</div>}
          {unplan.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Remove failed: {String(unplan.error)}</div>}
          {reorderStories.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Story reorder failed: {String(reorderStories.error)}</div>}
          {reorderEpics.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Epic reorder failed: {String(reorderEpics.error)}</div>}
          <p style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 10 }}>
            Click + or drag a story into the sprint box below. Drag sprint stories back to the backlog column or click Remove to unassign them.
          </p>
          <SprintDropZone stories={targetStories} targetSprint={targetSprint} disabled={!targetSprint || plan.isPending || unplan.isPending} onRemove={unplanStory} onOpen={handleOpen} />
        </aside>
      </div>
      <DragOverlay dropAnimation={null}>
        {activeStory && (
          <div data-testid="backlog-drag-overlay">
            <div className="card card--overlay" style={{ marginBottom: 0 }}>
              <BacklogStoryContent story={activeStory} />
            </div>
          </div>
        )}
        {!activeStory && activeEpic && <EpicDragOverlay epic={activeEpic} />}
      </DragOverlay>
      {open && <StoryModal story={open} onClose={() => setOpen(null)} />}
    </DndContext>
  );
}
