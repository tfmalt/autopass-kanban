import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Story, StoryDetail } from "@shared/generated/api.js";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { StoryModal, type StoryStatusOption } from "./StoryModal.js";

const hooks = vi.hoisted(() => ({
  useConfig: vi.fn(),
  useRepository: vi.fn(),
  useTeam: vi.fn(),
  useStory: vi.fn(),
  useUpdateStory: vi.fn(),
  useUpdateStoryFields: vi.fn(),
  useCreateTask: vi.fn(),
  useUpdateTask: vi.fn(),
  useDeleteTask: vi.fn(),
  useReorderTasks: vi.fn(),
}));

vi.mock("../api/hooks.js", () => hooks);

function baseStory(): Story {
  return {
    id: "US-F1-061",
    title: "Frontend story",
    status: "in-progress",
    phase: "F1",
    epic: "EP-F1-07",
    sprint: "S000.start",
    priority: null,
    storyPoints: 5,
    assignee: "Old Assignee <old@example.com>",
    assignees: ["Old Assignee <old@example.com>"],
    workStarted: null,
    workDone: null,
    activated: null,
    created: null,
    updated: null,
    relativePath: "delivery/backlog/story.md",
    tasks: [],
    taskSummary: { todo: 0, inProgress: 0, readyForQa: 0, done: 0, blocked: 0, total: 0 },
    frontmatter: {},
  };
}

function baseDetail(overrides: Partial<StoryDetail> = {}): StoryDetail {
  const story = baseStory();
  return {
    ...story,
    body: "# Story body",
    ...overrides,
  };
}

describe("StoryModal", () => {
  beforeEach(() => {
    hooks.useRepository.mockReturnValue({
      data: {
        sprints: [
          { name: "S000.start" },
          { name: "S001.next" },
        ],
      },
    });
    hooks.useConfig.mockReturnValue({
      data: {
        port: 3000,
        host: "127.0.0.1",
        style: "calm-light",
        version: "test",
        branch: "test",
        storyPoints: { allowedValues: ["1", "2", "3", "5", "8", "13"], aliases: {} },
      },
    });
    hooks.useTeam.mockReturnValue({
      data: [
        { name: "Erik Itland", email: "erik.vardal.itland@vegvesen.no", label: "Erik Itland <erik.vardal.itland@vegvesen.no>" },
        { name: "Sondre Bjerkerud", email: "sondre.bjerkerud@vegvesen.no", label: "Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>" },
      ],
    });
    hooks.useStory.mockReturnValue({
      data: baseDetail(),
      isLoading: false,
      isError: false,
    });
    hooks.useUpdateStory.mockReturnValue({
      isPending: false,
      mutate: vi.fn((_vars, options) => options?.onSuccess?.()),
    });
    hooks.useUpdateStoryFields.mockReturnValue({
      isPending: false,
      mutate: vi.fn((_vars, options) => options?.onSuccess?.()),
    });
    hooks.useCreateTask.mockReturnValue({
      isPending: false,
      mutate: vi.fn((_vars, options) => options?.onSuccess?.()),
    });
    hooks.useUpdateTask.mockReturnValue({
      isPending: false,
      mutate: vi.fn((_vars, options) => options?.onSuccess?.()),
    });
    hooks.useDeleteTask.mockReturnValue({
      isPending: false,
      mutate: vi.fn((_vars, options) => options?.onSuccess?.()),
    });
    hooks.useReorderTasks.mockReturnValue({
      isPending: false,
      mutate: vi.fn((_vars, options) => options?.onSuccess?.()),
    });
  });

  it("renders team members in the assignee datalist during edit mode", () => {
    const { container } = render(<StoryModal story={baseStory()} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    const options = Array.from(
      container.querySelectorAll<HTMLDataListElement>("#story-panel-team-list option"),
    ).map((option) => option.getAttribute("value"));

    expect(options).toContain("Erik Itland <erik.vardal.itland@vegvesen.no>");
    expect(options).toContain("Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>");
  });

  it("shows the live detail assignee instead of the stale board snapshot", () => {
    hooks.useStory.mockReturnValue({
      data: baseDetail({ assignee: "New Assignee <new@example.com>", assignees: ["New Assignee <new@example.com>"] }),
      isLoading: false,
      isError: false,
    });

    render(<StoryModal story={baseStory()} onClose={vi.fn()} />);

    expect(screen.getByText("New Assignee <new@example.com> · Epic: EP-F1-07 · Sprint: S000.start")).toBeInTheDocument();
    expect(screen.queryByText("Old Assignee <old@example.com> · Epic: EP-F1-07 · Sprint: S000.start")).not.toBeInTheDocument();
  });

  it("autocompletes assignees on Tab and accepts multiple assignees", () => {
    const { container } = render(<StoryModal story={baseStory()} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    const assignee = screen.getByRole("combobox", { name: /assignee/i }) as HTMLInputElement;
    fireEvent.change(assignee, { target: { value: "Son" } });
    assignee.setSelectionRange(3, 3);
    fireEvent.keyDown(assignee, { key: "Tab" });

    expect(assignee).toHaveValue("Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>, ");

    fireEvent.change(assignee, {
      target: { value: "Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>, Er" },
    });
    assignee.setSelectionRange(assignee.value.length, assignee.value.length);
    fireEvent.keyDown(assignee, { key: "Tab" });

    expect(assignee).toHaveValue(
      "Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>, Erik Itland <erik.vardal.itland@vegvesen.no>, ",
    );
    expect(Array.from(container.querySelectorAll<HTMLDataListElement>("#story-panel-team-list option")).map((option) => option.getAttribute("value"))).toEqual(
      expect.arrayContaining([
        "Erik Itland <erik.vardal.itland@vegvesen.no>",
        "Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>",
      ]),
    );
  });

  it("saves status, story points, and multiple assignees", async () => {
    const updateFields = vi.fn((_vars, options) => options?.onSuccess?.());
    hooks.useUpdateStoryFields.mockReturnValue({ isPending: false, mutate: updateFields });

    render(<StoryModal story={baseStory()} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const storyPoints = screen.getByLabelText("Story points") as HTMLSelectElement;
    expect(Array.from(storyPoints.options).map((option) => option.value)).toEqual(["1", "2", "3", "5", "8", "13"]);
    fireEvent.change(screen.getByLabelText("Status"), { target: { value: "ready-for-qa" } });
    fireEvent.change(storyPoints, { target: { value: "13" } });
    fireEvent.change(screen.getByRole("combobox", { name: /assignee/i }), {
      target: {
        value:
          "Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>, Erik Itland <erik.vardal.itland@vegvesen.no>",
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(updateFields).toHaveBeenCalledWith(
        {
          id: "US-F1-061",
          fields: {
            status: "ready-for-qa",
            storyPoints: "13",
            assignee:
              "Sondre Bjerkerud <sondre.bjerkerud@vegvesen.no>, Erik Itland <erik.vardal.itland@vegvesen.no>",
          },
        },
        expect.any(Object),
      );
    });
  });

  it("supports caller-provided lifecycle status options", () => {
    const story = baseStory();
    story.status = "todo";
    const statusOptions: StoryStatusOption[] = [
      { value: "draft", label: "draft" },
      { value: "ready", label: "ready" },
      { value: "planned", label: "planned" },
    ];
    hooks.useStory.mockReturnValue({
      data: baseDetail({ status: "todo" }),
      isLoading: false,
      isError: false,
    });

    render(<StoryModal story={story} onClose={vi.fn()} statusOptions={statusOptions} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    const status = screen.getByLabelText("Status") as HTMLSelectElement;
    expect(Array.from(status.options).map((option) => option.textContent)).toEqual(["todo", "draft", "ready", "planned"]);
    expect(Array.from(status.options).map((option) => option.value)).toEqual(["todo", "draft", "ready", "planned"]);
  });

  it("edits all task fields from the selected task", async () => {
    const updateTask = vi.fn((_vars, options) => options?.onSuccess?.());
    const story = baseStory();
    story.tasks = [
      { id: "TASK-US-F1-061-001", title: "Wire status picker", status: "todo", tags: [], description: "" },
    ];
    story.taskSummary = { todo: 1, inProgress: 0, readyForQa: 0, done: 0, blocked: 0, total: 1 };
    hooks.useStory.mockReturnValue({
      data: baseDetail({ tasks: story.tasks, taskSummary: story.taskSummary }),
      isLoading: false,
      isError: false,
    });
    hooks.useUpdateTask.mockReturnValue({ isPending: false, mutate: updateTask });

    render(<StoryModal story={story} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /Wire status picker/ }));
    fireEvent.change(screen.getByLabelText("Title for TASK-US-F1-061-001"), { target: { value: "Update task form" } });
    fireEvent.change(screen.getByLabelText("Status for TASK-US-F1-061-001"), {
      target: { value: "done" },
    });
    fireEvent.change(screen.getByLabelText("Tags for TASK-US-F1-061-001"), { target: { value: "web, test" } });
    fireEvent.change(screen.getByLabelText("Description for TASK-US-F1-061-001"), { target: { value: "Updated details" } });
    fireEvent.click(screen.getByRole("button", { name: "Save task" }));

    await waitFor(() => {
      expect(updateTask).toHaveBeenCalledWith(
        { storyId: "US-F1-061", taskId: "TASK-US-F1-061-001", title: "Update task form", status: "done", tags: "web, test", description: "Updated details" },
        expect.any(Object),
      );
    });
  });

  it("creates and reorders tasks", async () => {
    const createTask = vi.fn((_vars, options) => options?.onSuccess?.());
    const reorderTasks = vi.fn((_vars, options) => options?.onSuccess?.());
    const story = baseStory();
    story.tasks = [
      { id: "TASK-US-F1-061-001", title: "First", status: "todo", tags: [], description: "" },
      { id: "TASK-US-F1-061-002", title: "Second", status: "todo", tags: [], description: "" },
    ];
    story.taskSummary = { todo: 2, inProgress: 0, readyForQa: 0, done: 0, blocked: 0, total: 2 };
    hooks.useStory.mockReturnValue({ data: baseDetail({ tasks: story.tasks, taskSummary: story.taskSummary }), isLoading: false, isError: false });
    hooks.useCreateTask.mockReturnValue({ isPending: false, mutate: createTask });
    hooks.useReorderTasks.mockReturnValue({ isPending: false, mutate: reorderTasks });

    render(<StoryModal story={story} onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Add task" }));
    fireEvent.change(screen.getByLabelText("Title for new task"), { target: { value: "Third" } });
    fireEvent.click(screen.getByRole("button", { name: "Create task" }));
    fireEvent.click(screen.getByRole("button", { name: "Move TASK-US-F1-061-002 up" }));

    await waitFor(() => {
      expect(createTask).toHaveBeenCalledWith(expect.objectContaining({ storyId: "US-F1-061", title: "Third", status: "todo" }), expect.any(Object));
      expect(reorderTasks).toHaveBeenCalledWith({ storyId: "US-F1-061", taskIds: ["TASK-US-F1-061-002", "TASK-US-F1-061-001"] }, expect.any(Object));
    });
  });

  it("confirms before deleting a task", async () => {
    const deleteTask = vi.fn((_vars, options) => options?.onSuccess?.());
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const story = baseStory();
    story.tasks = [{ id: "TASK-US-F1-061-001", title: "Delete me", status: "todo", tags: [], description: "" }];
    story.taskSummary = { todo: 1, inProgress: 0, readyForQa: 0, done: 0, blocked: 0, total: 1 };
    hooks.useStory.mockReturnValue({ data: baseDetail({ tasks: story.tasks, taskSummary: story.taskSummary }), isLoading: false, isError: false });
    hooks.useDeleteTask.mockReturnValue({ isPending: false, mutate: deleteTask });

    render(<StoryModal story={story} onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: /Delete me/ }));
    fireEvent.click(screen.getByRole("button", { name: "Delete task" }));

    await waitFor(() => {
      expect(deleteTask).toHaveBeenCalledWith({ storyId: "US-F1-061", taskId: "TASK-US-F1-061-001" }, expect.any(Object));
    });
  });
});
