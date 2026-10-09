import { useCallback, useLayoutEffect, useRef, useState } from "react";

/** Fixed-row-height windowing: only rows intersecting the viewport (+ overscan) are rendered. */
export function useVirtual(count: number, rowHeight: number, overscan = 8, enabled = true) {
  const ref = useRef<HTMLDivElement>(null);
  const [range, setRange] = useState({ start: 0, end: Math.min(count, 60) });

  const measure = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    const start = Math.max(0, Math.floor(el.scrollTop / rowHeight) - overscan);
    const end = Math.min(count, Math.ceil((el.scrollTop + el.clientHeight) / rowHeight) + overscan);
    setRange((r) => (r.start === start && r.end === end ? r : { start, end }));
  }, [count, rowHeight, overscan]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || !enabled) return;
    measure();
    el.addEventListener("scroll", measure, { passive: true });
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", measure);
      ro.disconnect();
    };
  }, [measure, enabled]);

  const start = enabled ? range.start : 0;
  const end = enabled ? Math.min(range.end, count) : count;
  return { ref, start, end, padTop: start * rowHeight, padBottom: (count - end) * rowHeight };
}
