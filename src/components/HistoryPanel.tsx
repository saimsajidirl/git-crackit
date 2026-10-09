import React, { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CommitInfo } from "../types";
import { Icon } from "../icons";
import { Avatar } from "./Avatar";
import { cx, timeAgo } from "../util";
import { useVirtual } from "../useVirtual";

const LANE_W = 14;
const ROW_H = 40;
const LANE_COLORS = ["#7b6cff", "#3fc8ff", "#3ddc97", "#ff5f7e", "#ffad5c", "#c48bff", "#f2cc60", "#2dd4bf", "#f472b6"];

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

const GraphCell = memo(function GraphCell({ g, isHead }: { g: RowGraph; isHead: boolean }) {
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
    if (a && b) segs.push(<line key={`v${j}`} x1={cx(j)} y1={0} x2={cx(j)} y2={ROW_H} stroke={c} strokeWidth={2} />);
    else if (a) segs.push(<line key={`v${j}`} x1={cx(j)} y1={0} x2={cx(j)} y2={mid} stroke={c} strokeWidth={2} />);
    else if (b) segs.push(<line key={`v${j}`} x1={cx(j)} y1={mid} x2={cx(j)} y2={ROW_H} stroke={c} strokeWidth={2} />);
  }
  for (const [f, t] of g.edges) {
    if (f === t) continue;
    segs.push(
      <path
        key={`e${f}-${t}`}
        d={`M ${cx(f)} ${mid} C ${cx(f)} ${ROW_H}, ${cx(t)} ${mid}, ${cx(t)} ${ROW_H}`}
        stroke={LANE_COLORS[t % LANE_COLORS.length]}
        strokeWidth={2}
        fill="none"
      />
    );
  }
  return (
    <svg width={w} height={ROW_H} className="graph-cell" aria-hidden>
      {segs}
      {isHead && <circle cx={cx(g.lane)} cy={mid} r={7.5} fill={color} opacity={0.22} />}
      <circle className="graph-node" cx={cx(g.lane)} cy={mid} r={isHead ? 5 : 4} fill={color} strokeWidth={2} />
    </svg>
  );
});

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

const CommitRow = memo(function CommitRow({
  c, g, selected, isHead, onSelect, onContextMenu,
}: {
  c: CommitInfo; g: RowGraph; selected: boolean; isHead: boolean;
  onSelect: (oid: string) => void; onContextMenu: (e: React.MouseEvent, c: CommitInfo) => void;
}) {
  return (
    <div
      className={cx("commit-row", selected && "selected")}
      style={{ height: ROW_H }}
      onClick={() => onSelect(c.oid)}
      onContextMenu={(e) => onContextMenu(e, c)}
    >
      <GraphCell g={g} isHead={isHead} />
      <Avatar name={c.author_name} email={c.author_email} size={22} />
      <div className="commit-main">
        <div className="commit-summary" title={c.summary}>
          {c.refs.map((r) => refBadge(r.kind, r.name, isHead))}
          <span>{c.summary || "(no message)"}</span>
        </div>
        <div className="commit-meta">
          <span className="author">{c.author_name}</span>
          <span className="sha">{c.short_id}</span>
          <span>{timeAgo(c.author_time)}</span>
        </div>
      </div>
    </div>
  );
});

export function HistoryPanel({
  commits,
  selectedOid,
  headOid,
  onSelect,
  onContextMenu,
  onSearch,
  onLoadMore,
  hasMore,
}: {
  commits: CommitInfo[];
  selectedOid: string | null;
  headOid: string | null;
  onSelect: (oid: string) => void;
  onContextMenu: (e: React.MouseEvent, c: CommitInfo) => void;
  onSearch: (q: string) => void;
  onLoadMore: () => Promise<void> | void;
  hasMore: boolean;
}) {
  const [query, setQuery] = useState("");
  const graph = useMemo(() => computeGraph(commits), [commits]);
  const maxLanes = useMemo(() => graph.reduce((m, g) => Math.max(m, g.laneCount), 1), [graph]);
  const graphW = maxLanes * LANE_W + 8;
  const { ref, start, end, padTop, padBottom } = useVirtual(commits.length, ROW_H, 10);

  // Stable callbacks so memoized rows don't re-render when the parent re-renders.
  const ctxRef = useRef(onContextMenu);
  ctxRef.current = onContextMenu;
  const onCtx = useCallback((e: React.MouseEvent, c: CommitInfo) => ctxRef.current(e, c), []);
  const selRef = useRef(onSelect);
  selRef.current = onSelect;
  const onSel = useCallback((oid: string) => selRef.current(oid), []);

  const searchTimer = useRef<number>();
  const onQuery = (q: string) => {
    setQuery(q);
    window.clearTimeout(searchTimer.current);
    searchTimer.current = window.setTimeout(() => onSearch(q), 160);
  };
  useEffect(() => () => window.clearTimeout(searchTimer.current), []);

  const loadingMore = useRef(false);
  useEffect(() => {
    if (!hasMore || loadingMore.current || commits.length === 0 || end < commits.length - 30) return;
    loadingMore.current = true;
    Promise.resolve(onLoadMore()).finally(() => { loadingMore.current = false; });
  }, [end, commits.length, hasMore, onLoadMore]);

  return (
    <div className="history-panel">
      <div className="search-wrap">
        <Icon name="search" size={14} className="search-icon" />
        <input
          className="input search"
          placeholder="Search commits, authors, SHA…"
          value={query}
          onChange={(e) => onQuery(e.target.value)}
        />
      </div>
      <div className="commit-list" ref={ref} style={{ ["--graphw" as string]: `${graphW}px` }}>
        {padTop > 0 && <div style={{ height: padTop }} />}
        {commits.slice(start, end).map((c, k) => {
          const i = start + k;
          return (
            <CommitRow
              key={c.oid}
              c={c}
              g={graph[i]}
              selected={selectedOid === c.oid}
              isHead={c.oid === headOid}
              onSelect={onSel}
              onContextMenu={onCtx}
            />
          );
        })}
        {padBottom > 0 && <div style={{ height: padBottom }} />}
        {commits.length === 0 && (
          <div className="list-empty">
            <span className="empty-icon"><Icon name="history" size={22} /></span>
            <p>{query ? "No matching commits" : "No commits yet"}</p>
          </div>
        )}
        {hasMore && commits.length > 0 && (
          <div className="load-more"><Icon name="sync" size={12} className="spin" /> Loading more commits…</div>
        )}
      </div>
    </div>
  );
}
