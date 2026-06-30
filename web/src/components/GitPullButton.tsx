import { useEffect, useRef, useState } from "react";
import { useGitPull } from "../api/hooks.js";

type PullState = "idle" | "loading" | "success" | "error";

const ICON_SIZE = 15;

function SyncIcon({ spin, color }: { spin?: boolean; color: string }) {
  return (
    <svg
      width={ICON_SIZE}
      height={ICON_SIZE}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={spin ? { animation: "git-pull-spin 0.9s linear infinite" } : undefined}
    >
      <path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" />
      <path d="M21 3v5h-5" />
    </svg>
  );
}

export function GitPullButton() {
  const pull = useGitPull();
  const [uiState, setUiState] = useState<PullState>("idle");
  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (pull.isPending) {
      setUiState("loading");
    } else if (pull.isSuccess) {
      if (pull.data.ok) {
        setUiState("success");
        successTimerRef.current = setTimeout(() => setUiState("idle"), 2500);
      } else {
        setUiState("error");
      }
    } else if (pull.isError) {
      setUiState("error");
    }
    return () => {
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
    };
  }, [pull.isPending, pull.isSuccess, pull.isError, pull.data]);

  const handleClick = () => {
    if (uiState === "loading") return;
    setUiState("idle");
    pull.mutate();
  };

  const errorMessage =
    pull.isError
      ? String(pull.error)
      : pull.isSuccess && !pull.data.ok
        ? pull.data.message
        : undefined;

  const tooltipLabel =
    uiState === "loading"
      ? "Syncing…"
      : uiState === "success"
        ? "Up to date"
        : uiState === "error" && errorMessage
          ? `Sync failed: ${errorMessage}`
          : "Pull latest data";

  const iconColor =
    uiState === "error"
      ? "var(--red)"
      : uiState === "success"
        ? "var(--green)"
        : "var(--text-faint)";

  return (
    <>
      <style>{`
        @keyframes git-pull-spin {
          from { transform: rotate(0deg); }
          to   { transform: rotate(360deg); }
        }
        .git-pull-btn {
          background: none;
          border: 1px solid transparent;
          border-radius: 7px;
          cursor: pointer;
          display: inline-flex;
          align-items: center;
          justify-content: center;
          padding: 5px 6px;
          transition: background 0.12s, border-color 0.12s;
          line-height: 0;
        }
        .git-pull-btn:hover:not(:disabled),
        .git-pull-btn:focus-visible {
          background: var(--surface-2);
          border-color: var(--border);
          outline: none;
        }
        .git-pull-btn:disabled { cursor: default; }
      `}</style>
      <button
        type="button"
        className="git-pull-btn"
        onClick={handleClick}
        disabled={uiState === "loading"}
        aria-label={tooltipLabel}
        title={tooltipLabel}
      >
        <SyncIcon spin={uiState === "loading"} color={iconColor} />
      </button>
    </>
  );
}
