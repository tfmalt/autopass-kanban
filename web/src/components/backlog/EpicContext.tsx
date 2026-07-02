import DOMPurify from "dompurify";
import { marked } from "marked";
import { useMemo } from "react";
import { useEpic } from "../../api/hooks.js";

interface EpicPreviewSection {
  heading: string;
  markdown: string;
}

marked.use({ gfm: true, breaks: false });

export function parseEpicPreview(body: string): EpicPreviewSection[] {
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

export function EpicContext({ epicId }: { epicId: string }) {
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
