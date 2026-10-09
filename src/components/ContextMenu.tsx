import React, { useEffect, useRef } from "react";

export interface MenuItem {
  label: string;
  onClick?: () => void;
  danger?: boolean;
  separator?: boolean;
  disabled?: boolean;
}

export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  // Clamp to viewport
  const style: React.CSSProperties = { left: x, top: y };
  const estimatedH = items.length * 28 + 8;
  if (y + estimatedH > window.innerHeight) style.top = Math.max(4, window.innerHeight - estimatedH - 4);
  if (x + 240 > window.innerWidth) style.left = window.innerWidth - 244;

  return (
    <div className="context-menu" style={style} ref={ref}>
      {items.map((it, i) =>
        it.separator ? (
          <div key={i} className="menu-separator" />
        ) : (
          <button
            key={i}
            className={"menu-item" + (it.danger ? " danger" : "")}
            disabled={it.disabled}
            onClick={() => {
              onClose();
              it.onClick?.();
            }}
          >
            {it.label}
          </button>
        )
      )}
    </div>
  );
}

/** A dropdown anchored under a button, same look as context menu. */
export function Dropdown({
  open,
  onClose,
  children,
  align = "left",
  width,
}: {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
  align?: "left" | "right";
  width?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div
      className="dropdown"
      ref={ref}
      style={{ [align === "right" ? "right" : "left"]: 0, width }}
    >
      {children}
    </div>
  );
}
