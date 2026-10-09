import React, { useMemo } from "react";
import type { FileDiff } from "../types";
import { Icon } from "../icons";
import { useVirtual } from "../useVirtual";

const LINE_H = 20;
const VIRTUALIZE_AFTER = 400;

interface Row {
  kind: string;
  old: number | null;
  new: number | null;
  text: string;
}

export function DiffEmpty({ icon, title, sub }: { icon: string; title: string; sub?: string }) {
  return (
    <div className="diff-empty">
      <span className="empty-icon"><Icon name={icon} size={24} /></span>
      <p>{title}</p>
      {sub && <p className="faint small">{sub}</p>}
    </div>
  );
}

export function DiffView({ diff }: { diff: FileDiff | null }) {
  const { rows, maxCols } = useMemo(() => {
    const rows: Row[] = [];
    let maxCols = 0;
    for (const h of diff?.hunks ?? []) {
      rows.push({ kind: "hunk", old: null, new: null, text: h.header.replace(/\n$/, "") });
      for (const l of h.lines) {
        const text = l.content.replace(/\n$/, "");
        rows.push({ kind: l.kind, old: l.old_lineno, new: l.new_lineno, text });
        const cols = text.length + (text.match(/\t/g)?.length ?? 0) * 3;
        if (cols > maxCols) maxCols = cols;
      }
    }
    return { rows, maxCols };
  }, [diff]);

  const virtual = rows.length > VIRTUALIZE_AFTER;
  const { ref, start, end, padTop, padBottom } = useVirtual(rows.length, LINE_H, 20, virtual);

  if (!diff) return <DiffEmpty icon="diff" title="Select a file to view its changes" />;
  if (diff.is_binary) return <DiffEmpty icon="file" title={diff.path} sub="Binary file — no preview available" />;
  if (diff.hunks.length === 0) return <DiffEmpty icon="file" title={diff.path} sub="No textual changes to display" />;

  return (
    <div className="diff-view" key={diff.path}>
      <div className="diff-header">
        <span className="diff-path" title={diff.path}>
          <Icon name="file" size={13} />
          <span className="ellipsis">{diff.path}</span>
        </span>
        <span className="diff-stats">
          <span className="stat-add">+{diff.additions}</span>
          <span className="stat-del">−{diff.deletions}</span>
        </span>
      </div>
      {diff.too_large && <div className="diff-note">Diff truncated — file is very large.</div>}
      <div className="diff-scroll" ref={ref}>
        <div className="diff-sizer" style={{ width: `calc(${maxCols}ch + 160px)` }}>
          {padTop > 0 && <div style={{ height: padTop }} />}
          {rows.slice(start, end).map((r, i) => (
            <div key={start + i} className={`dl ${r.kind}`}>
              <span className="dl-gutter">
                <span>{r.old ?? ""}</span>
                <span>{r.new ?? ""}</span>
              </span>
              {r.kind !== "hunk" && (
                <span className="dl-sign">{r.kind === "add" ? "+" : r.kind === "del" ? "−" : ""}</span>
              )}
              <span className="dl-text">{r.text}</span>
            </div>
          ))}
          {padBottom > 0 && <div style={{ height: padBottom }} />}
        </div>
      </div>
    </div>
  );
}
