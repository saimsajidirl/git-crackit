import React, { useMemo, useState } from "react";
import type { CommitInfo } from "../types";
import { Icon } from "../icons";
import { Avatar } from "./Avatar";
import { cx, timeAgo } from "../util";

const LANE_W = 14;
const ROW_H = 34;
const LANE_COLORS = ["#0969da", "#1a7f37", "#bf3989", "#bc4c00", "#8250df", "#0a7ea4", "#9a6700", "#cf222e", "#57606a"];

interface RowGraph {
  lane: number;
  laneCount: number;
  above: boolean[];
  below: boolean[];
  edges: [number, number][];
}

function computeGraph(commits: CommitInfo[]): RowGraph[] {
  let lanes: (string | null)[] = [];
  const rows: RowGraph[] = [];
  for (const c of commits) {
    const above = lanes.map((l) => l !== null);
    let lane = lanes.indexOf(c.oid);
    if (lane === -1) {
      lane = lanes.indexOf(null);
      if (lane === -1) {
        lane = lanes.length;
        lanes.push(null);
      } else {
        lanes[lane] = c.oid;
      }
    }
    lanes[lane] = null;
    const edges: [number, number][] = [];
    for (const p of c.parents) {
      let pl = lanes.indexOf(p);
      if (pl === -1) {
        pl = lanes.indexOf(null);
        if (pl === -1) {
          lanes.push(p);
          pl = lanes.length - 1;
        } else {
          lanes[pl] = p;
        }
      }
      edges.push([lane, pl]);
    }
    rows.push({ lane, laneCount: lanes.length, above, below: lanes.map((l) => l !== null), edges });
  }
  return rows;
}

function GraphCell({ g, isHead }: { g: RowGraph; isHead: boolean }) {
  const w = Math.max(g.laneCount * LANE_W + 8, 22);
  const cx = (l: number) => l * LANE_W + 8;
  const mid = ROW_H / 2;
  const color = LANE_COLORS[g.lane % LANE_COLORS.length];
  const segs: React.ReactNode[] = [];

  const n = Math.max(g.laneCount, g.above.length, g.below.length);
  for (let j = 0; j < n; j++) {
    const a = g.above[j] || j === g.lane;
    const b = g.below[j];
    const c = LANE_COLORS[j % LANE_COLORS.length];
    if (a && b) segs.push(<line key={`v${j}`} x1={cx(j)} y1={0} x2={cx(j)} y2={ROW_H} stroke={c} strokeWidth={1.6} />);
    else if (a) segs.push(<line key={`v${j}`} x1={cx(j)} y1={0} x2={cx(j)} y2={mid} stroke={c} strokeWidth={1.6} />);
    else if (b) segs.push(<line key={`v${j}`} x1={cx(j)} y1={mid} x2={cx(j)} y2={ROW_H} stroke={c} strokeWidth={1.6} />);
  }
  for (const [f, t] of g.edges) {
    if (f === t) continue;
    segs.push(
      <path
        key={`e${f}-${t}`}
        d={`M ${cx(f)} ${mid} C ${cx(f)} ${ROW_H}, ${cx(t)} ${mid}, ${cx(t)} ${ROW_H}`}
        stroke={LANE_COLORS[t % LANE_COLORS.length]}
        strokeWidth={1.6}
        fill="none"
      />
    );
  }
  return (
    <svg width={w} height={ROW_H} className="graph-cell" aria-hidden>
      {segs}
      <circle cx={cx(g.lane)} cy={mid} r={isHead ? 4.6 : 3.6} fill={color} stroke="#fff" strokeWidth={1.4} />
    </svg>
  );
}

function refBadge(kind: string, name: string, isHeadRef: boolean) {
  const cls = kind === "tag" ? "tag-badge" : kind === "remote" ? "remote-badge" : "branch-badge";
  const icon = kind === "tag" ? "tag" : "branch";
  return (
    <span key={kind + name} className={`ref-badge ${cls}${isHeadRef ? " head" : ""}`} title={name}>
      <Icon name={icon} size={11} />
      {name}
    </span>
  );
}

export function HistoryPanel({
  commits,
  selectedOid,
  headOid,
  onSelect,
  onContextMenu,
  onSearch,
  onLoadMore,
  hasMore,
  loading,
}: {
  commits: CommitInfo[];
  selectedOid: string | null;
  headOid: string | null;
  onSelect: (oid: string) => void;
  onContextMenu: (e: React.MouseEvent, c: CommitInfo) => void;
  onSearch: (q: string) => void;
  onLoadMore: () => void;
  hasMore: boolean;
  loading: boolean;
}) {
  const [query, setQuery] = useState("");
  const graph = useMemo(() => computeGraph(commits), [commits]);
  const maxLanes = useMemo(() => Math.max(1, ...graph.map((g) => g.laneCount)), [graph]);
  const graphW = maxLanes * LANE_W + 8;

  return (
    <div className="history-panel">
      <div className="panel-header">
        <span className="panel-title">History</span>
      </div>
      <div className="search-wrap">
        <Icon name="search" size={14} className="search-icon" />
        <input
          className="input search"
          placeholder="Search commits, authors, SHA…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            onSearch(e.target.value);
          }}
        />
      </div>
      <div className="commit-list" style={{ ["--graphw" as string]: `${graphW}px` }}>
        {commits.map((c, i) => {
          const g = graph[i];
          return (
            <div
              key={c.oid}
              className={cx("commit-row", selectedOid === c.oid && "selected")}
              style={{ height: ROW_H }}
              onClick={() => onSelect(c.oid)}
              onContextMenu={(e) => onContextMenu(e, c)}
            >
              <GraphCell g={g} isHead={c.oid === headOid} />
              <Avatar name={c.author_name} email={c.author_email} size={22} />
              <div className="commit-main">
                <div className="commit-summary" title={c.summary}>
                  {c.refs.map((r) => refBadge(r.kind, r.name, c.oid === headOid))}
                  <span>{c.summary || "(no message)"}</span>
                </div>
                <div className="commit-meta">
                  <span className="muted">{c.author_name}</span>
                  <span className="sha">{c.short_id}</span>
                  <span className="muted">{timeAgo(c.author_time)}</span>
                </div>
              </div>
            </div>
          );
        })}
        {commits.length === 0 && !loading && (
          <div className="list-empty">
            <Icon name="history" size={28} />
            <p>No commits yet</p>
          </div>
        )}
        {hasMore && (
          <button className="link-btn load-more" onClick={onLoadMore} disabled={loading}>
            {loading ? "Loading…" : "Load more commits"}
          </button>
        )}
      </div>
    </div>
  );
}
