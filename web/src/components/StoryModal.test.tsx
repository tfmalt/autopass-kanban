import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Story, StoryDetail } from "@shared/types.js";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { StoryModal, type StoryStatusOption } from "./StoryModal.js";

const hooks = vi.hoisted(() => ({
  useConfig: vi.fn(),
  useRepository: vi.fn(),
  useTeam: vi.fn(),
  useStory: vi.fn(),
  useUpdateStory: vi.fn(),
  useUpdateStoryFields: vi.fn(),
  useUpdateTaskStatus: vi.fn(),
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
    hooks.useUpdateTaskStatus.mockReturnValue({
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
      { value: "todo", label: "planned" },
    ];
    hooks.useStory.mockReturnValue({
      data: baseDetail({ status: "todo" }),
      isLoading: false,
      isError: false,
    });

    render(<StoryModal story={story} onClose={vi.fn()} statusOptions={statusOptions} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    const status = screen.getByLabelText("Status") as HTMLSelectElement;
    expect(Array.from(status.options).map((option) => option.textContent)).toEqual(["draft", "ready", "planned"]);
    expect(Array.from(status.options).map((option) => option.value)).toEqual(["draft", "ready", "todo"]);
  });

  it("opens a task status picker and updates the selected task", async () => {
    const updateTaskStatus = vi.fn((_vars, options) => options?.onSuccess?.());
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
    hooks.useUpdateTaskStatus.mockReturnValue({ isPending: false, mutate: updateTaskStatus });

    render(<StoryModal story={story} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /TASK-US-F1-061-001/ }));
    fireEvent.change(screen.getByLabelText("Update status for TASK-US-F1-061-001"), {
      target: { value: "done" },
    });

    await waitFor(() => {
      expect(updateTaskStatus).toHaveBeenCalledWith(
        { storyId: "US-F1-061", taskId: "TASK-US-F1-061-001", status: "done" },
        expect.any(Object),
      );
    });
  });
});
