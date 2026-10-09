import React, { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../icons";
import { cx } from "../util";

export interface Command {
  id: string;
  label: string;
  group: string;
  icon?: string;
  hint?: string;
  run: () => void;
}

export function CommandPalette({ commands, onClose }: { commands: Command[]; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const filtered = useMemo(() => {
    const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
    if (!terms.length) return commands;
    return commands.filter((c) => {
      const hay = `${c.group} ${c.label}`.toLowerCase();
      return terms.every((t) => hay.includes(t));
    });
  }, [q, commands]);

  useEffect(() => setActive(0), [q]);
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-idx="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const exec = (c: Command | undefined) => {
    if (!c) return;
    onClose();
    c.run();
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(filtered.length - 1, a + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(0, a - 1)); }
    else if (e.key === "Enter") { e.preventDefault(); exec(filtered[active]); }
    else if (e.key === "Escape") { e.preventDefault(); onClose(); }
  };

  let lastGroup = "";
  return (
    <div className="modal-overlay palette-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="palette" role="dialog" aria-label="Command palette" onKeyDown={onKey}>
        <div className="palette-input-wrap">
          <Icon name="search" size={16} />
          <input
            className="palette-input"
            autoFocus
            placeholder="Type a command or search branches…"
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
          <kbd>Esc</kbd>
        </div>
        <div className="palette-list" ref={listRef}>
          {filtered.length === 0 && <div className="list-empty small-pad"><p>No matching commands</p></div>}
          {filtered.map((c, i) => {
            const header = c.group !== lastGroup ? <div className="palette-group">{c.group}</div> : null;
            lastGroup = c.group;
            return (
              <React.Fragment key={c.id}>
                {header}
                <button
                  data-idx={i}
                  className={cx("palette-item", i === active && "active")}
                  onMouseMove={() => i !== active && setActive(i)}
                  onClick={() => exec(c)}
                >
                  <Icon name={c.icon ?? "dot"} size={15} />
                  <span className="grow ellipsis">{c.label}</span>
                  {c.hint && <kbd>{c.hint}</kbd>}
                </button>
              </React.Fragment>
            );
          })}
        </div>
        <div className="palette-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
          <span><kbd><Icon name="enter" size={10} /></kbd> run</span>
          <span><kbd>Esc</kbd> close</span>
        </div>
      </div>
    </div>
  );
}
