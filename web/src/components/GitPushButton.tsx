import { useEffect, useRef, useState } from "react";
import type { GitStatusResponse } from "@shared/generated/api.js";
import { useGitPush } from "../api/hooks.js";

type PushState = "idle" | "loading" | "success" | "error";

function PushIcon({ spin, color }: { spin?: boolean; color: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={spin ? { animation: "git-pull-spin 0.9s linear infinite" } : undefined}>
      <path d="M12 15V4" />
      <path d="m7 9 5-5 5 5" />
      <path d="M4 20h16" />
    </svg>
  );
}

export function GitPushButton({ status }: { status?: GitStatusResponse }) {
  const push = useGitPush();
  const [uiState, setUiState] = useState<PushState>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (push.isPending) setUiState("loading");
    else if (push.isSuccess) {
      if (push.data.ok) {
        setUiState("success");
        timer.current = setTimeout(() => setUiState("idle"), 2500);
      } else setUiState("error");
    } else if (push.isError) setUiState("error");
    return () => { if (timer.current) clearTimeout(timer.current); };
  }, [push.isPending, push.isSuccess, push.isError, push.data]);

  const unavailable = !status?.available;
  const noUpstream = status?.available && !status.upstream;
  const remoteAhead = status?.available && status.behind > 0;
  const nothingToPush = status?.available && status.pendingCount === 0 && status.ahead === 0;
  const disabled = uiState === "loading" || unavailable || noUpstream || remoteAhead || nothingToPush;
  const error = push.isError ? String(push.error) : push.isSuccess && !push.data.ok ? push.data.message : undefined;
  const label = uiState === "loading" ? "Pushing changes..." : uiState === "success" ? "Changes pushed" : uiState === "error" && error ? `Push failed: ${error}` : unavailable ? "Push unavailable: not a git repository" : noUpstream ? "Push unavailable: no remote tracking branch" : remoteAhead ? "Pull latest data before pushing" : nothingToPush ? "Nothing to commit or push" : "Commit and push web changes";
  const color = uiState === "error" ? "var(--red)" : uiState === "success" ? "var(--green)" : "var(--text-faint)";
  return <button type="button" className="git-sync-btn" onClick={() => { setUiState("idle"); push.mutate(); }} disabled={disabled} aria-label={label} title={label}><PushIcon spin={uiState === "loading"} color={color} /></button>;
}
