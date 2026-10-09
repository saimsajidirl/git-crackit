import React from "react";
import type { FileDiff } from "../types";
import { Icon } from "../icons";

export function DiffView({ diff }: { diff: FileDiff | null }) {
  if (!diff) {
    return (
      <div className="diff-empty">
        <Icon name="diff" size={32} />
        <p>Select a file to view its changes</p>
      </div>
    );
  }
  if (diff.is_binary) {
    return (
      <div className="diff-empty">
        <Icon name="file" size={32} />
        <p>{diff.path}</p>
        <p className="muted">Binary file — no preview available</p>
      </div>
    );
  }
  if (diff.hunks.length === 0) {
    return (
      <div className="diff-empty">
        <Icon name="file" size={32} />
        <p>{diff.path}</p>
        <p className="muted">No textual changes to display</p>
      </div>
    );
  }
  return (
    <div className="diff-scroll">
      <div className="diff-header">
        <span className="diff-path" title={diff.path}>{diff.path}</span>
        <span className="diff-stats">
          <span className="stat-add">+{diff.additions}</span>
          <span className="stat-del">−{diff.deletions}</span>
        </span>
      </div>
      {diff.too_large && (
        <div className="diff-note">Diff truncated — file is very large.</div>
      )}
      <table className="diff-table">
        <tbody>
          {diff.hunks.map((h, hi) => (
            <React.Fragment key={hi}>
              <tr className="hunk-header">
                <td className="lineno" colSpan={2}>
                  <span className="hunk-label">{h.header}</span>
                </td>
                <td className="line-content" />
              </tr>
              {h.lines.map((l, li) => (
                <tr key={li} className={`line-${l.kind}`}>
                  <td className="lineno old">{l.old_lineno ?? ""}</td>
                  <td className="lineno new">{l.new_lineno ?? ""}</td>
                  <td className="line-content">
                    <span className="line-sign">
                      {l.kind === "add" ? "+" : l.kind === "del" ? "−" : " "}
                    </span>
                    <span className="line-text">{l.content.replace(/\n$/, "")}</span>
                  </td>
                </tr>
              ))}
            </React.Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}
