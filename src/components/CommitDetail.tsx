import React, { useEffect, useState } from "react";
import type { CommitDetail, FileDiff } from "../types";
import { api } from "../api";
import { Icon } from "../icons";
import { Avatar } from "./Avatar";
import { DiffView } from "./DiffView";
import { cx, fileName, dirName, formatDate, statusLetter } from "../util";

export function CommitDetailView({
  detail,
  onError,
}: {
  detail: CommitDetail | null;
  onError: (msg: string) => void;
}) {
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);

  useEffect(() => {
    setSelectedFile(null);
    setDiff(null);
    if (detail && detail.files.length > 0) {
      const first = detail.files[0].path;
      setSelectedFile(first);
      api.getCommitFileDiff(detail.commit.oid, first).then(setDiff).catch((e) => onError(String(e)));
    }
  }, [detail?.commit.oid]);

  if (!detail) {
    return (
      <div className="diff-empty">
        <Icon name="history" size={32} />
        <p>Select a commit to view its details</p>
      </div>
    );
  }

  const c = detail.commit;
  const totalAdd = detail.files.reduce((s, f) => s + f.additions, 0);
  const totalDel = detail.files.reduce((s, f) => s + f.deletions, 0);

  return (
    <div className="commit-detail">
      <div className="commit-detail-header">
        <div className="cdh-top">
          <Avatar name={c.author_name} email={c.author_email} size={36} />
          <div className="cdh-text">
            <div className="cdh-summary">{c.summary}</div>
            <div className="cdh-meta muted">
              {c.author_name} committed {formatDate(c.author_time)} · <span className="sha">{c.oid}</span>
            </div>
            {c.parents.length > 0 && (
              <div className="cdh-meta muted">
                {c.parents.length} parent{c.parents.length > 1 ? "s" : ""}: {c.parents.map((p) => p.slice(0, 7)).join(", ")}
              </div>
            )}
          </div>
        </div>
        {c.body && <pre className="cdh-body">{c.body}</pre>}
      </div>
      <div className="commit-detail-body">
        <div className="commit-files">
          <div className="files-header muted">
            {detail.files.length} file{detail.files.length === 1 ? "" : "s"} changed
            <span className="diff-stats">
              <span className="stat-add">+{totalAdd}</span>
              <span className="stat-del">−{totalDel}</span>
            </span>
          </div>
          {detail.files.map((f) => (
            <button
              key={f.path}
              className={cx("file-row commit-file", selectedFile === f.path && "selected")}
              onClick={() => {
                setSelectedFile(f.path);
                api.getCommitFileDiff(c.oid, f.path).then(setDiff).catch((e) => onError(String(e)));
              }}
            >
              <span className={`status-badge st-${f.status}`}>{statusLetter(f.status)}</span>
              <span className="file-path" title={f.path}>
                <span className="file-name">{fileName(f.path)}</span>
                <span className="file-dir">{dirName(f.path)}</span>
              </span>
              <span className="diff-stats">
                {f.additions > 0 && <span className="stat-add">+{f.additions}</span>}
                {f.deletions > 0 && <span className="stat-del">−{f.deletions}</span>}
              </span>
            </button>
          ))}
        </div>
        <div className="commit-diff">
          <DiffView diff={diff} />
        </div>
      </div>
    </div>
  );
}
