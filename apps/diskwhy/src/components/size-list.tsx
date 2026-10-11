import {
  Button,
  cn,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@nova/ui";
import { ChevronRightIcon, FileIcon, FolderIcon, FolderOpenIcon, LockIcon, MoreHorizontalIcon, Trash2Icon } from "lucide-react";
import { CATEGORY_META, formatBytes, percent, tileColor } from "../lib/format";
import type { ChildView } from "../lib/types";

interface Props {
  items: ChildView[];
  total: number;
  onOpen: (c: ChildView) => void;
  onReveal: (c: ChildView) => void;
  onTrash: (c: ChildView) => void;
  selected?: string | null;
  onSelect?: (c: ChildView) => void;
}

export function SizeList({ items, total, onOpen, onReveal, onTrash, selected, onSelect }: Props) {
  const max = items[0]?.size ?? 1;
  return (
    <ul className="divide-y" aria-label="รายการเรียงตามขนาด">
      {items.map((c, i) => {
        const actionable = c.kind === "dir" || c.kind === "file";
        const Icon = c.kind === "dir" ? (c.denied ? LockIcon : FolderIcon) : FileIcon;
        return (
          <li
            key={`${c.kind}-${c.name}`}
            className={cn("group flex items-center gap-3 px-3 py-2 hover:bg-muted/60", selected === c.name && "bg-accent")}
          >
            <button
              type="button"
              disabled={!actionable}
              onClick={() => (c.kind === "dir" ? onOpen(c) : onSelect?.(c))}
              className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded text-left focus-visible:ring-[3px] focus-visible:ring-ring/45 focus-visible:outline-none disabled:cursor-default"
            >
              <span className="grid size-8 shrink-0 place-items-center rounded-md text-white" style={{ background: tileColor(c.kind, c.category, i) }}>
                <Icon className={cn("size-4", c.kind === "small" || c.kind === "rest" ? "text-muted-foreground" : "")} aria-hidden />
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-center gap-1 truncate text-sm font-medium">
                  {c.name}
                  {c.kind === "dir" && <ChevronRightIcon className="size-3.5 shrink-0 text-muted-foreground opacity-0 group-hover:opacity-100" aria-hidden />}
                </span>
                <span className="mt-1 block h-1.5 overflow-hidden rounded-full bg-muted">
                  <span className="block h-full rounded-full bg-primary/80" style={{ width: `${Math.max(1, (c.size / max) * 100)}%` }} />
                </span>
                <span className="mt-0.5 block truncate text-xs text-muted-foreground">
                  {c.kind === "dir"
                    ? c.denied
                      ? "ไม่มีสิทธิ์อ่าน"
                      : `${c.files.toLocaleString("th-TH")} ไฟล์`
                    : c.category
                      ? CATEGORY_META[c.category].label
                      : "รวมไฟล์เล็กที่ต่ำกว่า 1 MB"}
                </span>
              </span>
              <span className="w-24 shrink-0 text-right">
                <span className="block text-sm font-semibold tabular-nums">{formatBytes(c.size)}</span>
                <span className="block text-xs text-muted-foreground tabular-nums">{percent(c.size, total).toFixed(1)}%</span>
              </span>
            </button>
            {actionable ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button size="icon-sm" variant="ghost" aria-label={`ตัวเลือกสำหรับ ${c.name}`}>
                    <MoreHorizontalIcon />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-52">
                  {c.kind === "dir" && (
                    <DropdownMenuItem onSelect={() => onOpen(c)}>
                      <FolderOpenIcon /> เปิดดูข้างใน
                    </DropdownMenuItem>
                  )}
                  <DropdownMenuItem onSelect={() => onReveal(c)}>
                    <FolderIcon /> แสดงใน File Explorer
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem variant="destructive" onSelect={() => onTrash(c)}>
                    <Trash2Icon /> ย้ายไปถังขยะ…
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <span className="size-8 shrink-0" />
            )}
          </li>
        );
      })}
    </ul>
  );
}
