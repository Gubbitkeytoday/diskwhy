import { cn } from "@nova/ui";
import { hierarchy, treemap, treemapSquarify } from "d3-hierarchy";
import { useEffect, useMemo, useRef, useState } from "react";
import { formatBytes, percent, tileColor } from "../lib/format";
import type { ChildView } from "../lib/types";

interface Props {
  items: ChildView[];
  total: number;
  onOpen: (child: ChildView) => void;
  onSelect?: (child: ChildView) => void;
  selected?: string | null;
}

/** Squarified treemap: tile area ∝ size. Click a folder to drill in. Tiles are real buttons (keyboard + screen reader). */
export function Treemap({ items, total, onOpen, onSelect, selected }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [hover, setHover] = useState<ChildView | null>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => {
      if (e) setBox({ w: Math.floor(e.contentRect.width), h: Math.floor(e.contentRect.height) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const tiles = useMemo(() => {
    if (!box.w || !box.h) return [];
    const data = items.filter((c) => c.size > 0);
    const root = hierarchy<{ children?: ChildView[] } | ChildView>({ children: data }, (d) => ("children" in d ? d.children : undefined))
      .sum((d) => ("size" in d ? d.size : 0))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    treemap<{ children?: ChildView[] } | ChildView>().tile(treemapSquarify.ratio(1.25)).size([box.w, box.h]).paddingInner(3).round(true)(root);
    return root.leaves().map((l, i) => ({
      child: l.data as ChildView,
      x: (l as unknown as { x0: number }).x0,
      y: (l as unknown as { y0: number }).y0,
      w: (l as unknown as { x1: number }).x1 - (l as unknown as { x0: number }).x0,
      h: (l as unknown as { y1: number }).y1 - (l as unknown as { y0: number }).y0,
      i,
    }));
  }, [items, box]);

  const info = hover ?? items.find((c) => c.name === selected) ?? null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div ref={ref} className="relative min-h-0 flex-1 overflow-hidden rounded-lg bg-muted/40" role="group" aria-label="แผนผังขนาดไฟล์และโฟลเดอร์">
        {tiles.map(({ child, x, y, w, h, i }) => {
          const big = w > 70 && h > 34;
          const isDir = child.kind === "dir";
          const muted = child.kind === "small" || child.kind === "rest";
          return (
            <button
              key={`${child.kind}-${child.name}`}
              type="button"
              title={`${child.name} — ${formatBytes(child.size)}`}
              aria-label={`${isDir ? "โฟลเดอร์" : "ไฟล์"} ${child.name} ${formatBytes(child.size)}${isDir ? " กดเพื่อเปิด" : ""}`}
              onMouseEnter={() => setHover(child)}
              onMouseLeave={() => setHover(null)}
              onFocus={() => setHover(child)}
              onBlur={() => setHover(null)}
              onClick={() => (isDir ? onOpen(child) : onSelect?.(child))}
              disabled={muted}
              className={cn(
                "absolute flex flex-col items-start justify-start overflow-hidden rounded-md p-2 text-left transition-[filter,box-shadow,opacity] duration-150",
                "hover:brightness-110 focus-visible:z-10 focus-visible:ring-[3px] focus-visible:ring-ring focus-visible:outline-none",
                isDir ? "cursor-zoom-in text-white" : muted ? "cursor-default text-muted-foreground" : "cursor-pointer text-white",
                selected === child.name && "ring-[3px] ring-foreground",
              )}
              style={{
                left: x,
                top: y,
                width: w,
                height: h,
                background: tileColor(child.kind, child.category, i),
                boxShadow: "inset 0 -24px 32px -24px rgb(0 0 0 / 0.25)",
              }}
            >
              {big && (
                <>
                  <span className="w-full truncate text-xs font-semibold drop-shadow-sm">{child.name}</span>
                  <span className="text-[0.7rem] opacity-90 tabular-nums drop-shadow-sm">{formatBytes(child.size)}</span>
                </>
              )}
            </button>
          );
        })}
        {!items.length && <div className="grid h-full place-items-center text-sm text-muted-foreground">โฟลเดอร์ว่าง</div>}
      </div>
      <div className="flex h-6 items-center gap-2 truncate text-xs text-muted-foreground" aria-live="polite">
        {info ? (
          <>
            <span className="font-medium text-foreground">{info.name}</span>
            <span className="tabular-nums">{formatBytes(info.size)}</span>
            <span className="tabular-nums">· {percent(info.size, total).toFixed(1)}%</span>
            {info.kind === "dir" && <span>· {info.files.toLocaleString("th-TH")} ไฟล์ · คลิกเพื่อเปิด</span>}
          </>
        ) : (
          <span>ชี้เมาส์ที่ช่องเพื่อดูรายละเอียด · ช่องยิ่งใหญ่ = ยิ่งกินพื้นที่</span>
        )}
      </div>
    </div>
  );
}
