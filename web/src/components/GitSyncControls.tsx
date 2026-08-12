import { useGitStatus } from "../api/hooks.js";
import { GitPullButton } from "./GitPullButton.js";
import { GitPushButton } from "./GitPushButton.js";

export function GitSyncControls() {
  const status = useGitStatus();
  const data = status.data;
  const labels = [
    data?.behind ? `${data.behind} to pull` : "",
    data?.pendingCount ? `${data.pendingCount} to commit` : "",
    data?.ahead ? `${data.ahead} to push` : "",
  ].filter(Boolean);
  return (
    <div className="git-sync-controls">
      <div className="git-sync-buttons"><GitPullButton /><GitPushButton status={data} /></div>
      {labels.length > 0 && <span className="git-sync-summary" title={labels.join(" · ")}>{labels.join(" · ")}</span>}
    </div>
  );
}
