import { useRef, useState } from "react";
import { useEntry } from "@/api/hooks";
import { cn } from "@/lib/utils";
import { type EntryRow, EntrySummary } from "./composed/EntrySummary";
import { MarkdownContent } from "./composed/MarkdownContent";
import { EntryEditor } from "./EntryEditor";

function ExpandedBody({
  entryId,
  containerRef,
}: {
  entryId: string;
  containerRef: React.RefObject<HTMLElement | null>;
}) {
  const { data: detail, isLoading } = useEntry(entryId);
  const [editing, setEditing] = useState(false);

  const scrollBackIntoView = () => {
    setEditing(false);
    requestAnimationFrame(() => {
      const el = containerRef.current;
      if (!el) return;
      const header = document.querySelector("header");
      const headerHeight = header?.getBoundingClientRect().height ?? 0;
      const rect = el.getBoundingClientRect();
      if (rect.top < headerHeight) {
        window.scrollBy({ top: rect.top - headerHeight - 12, behavior: "smooth" });
      }
    });
  };

  if (isLoading || !detail) {
    return <div className="pt-3 text-xs text-muted-foreground">Loading...</div>;
  }

  if (editing) {
    return (
      <EntryEditor entry={detail} onCancel={() => setEditing(false)} onSaved={scrollBackIntoView} />
    );
  }

  return (
    <div className="border-t border-border/50 pt-3" onClick={(e) => e.stopPropagation()}>
      <div className="flex items-center justify-between">
        <div />
        <button
          type="button"
          onClick={() => setEditing(true)}
          className="rounded-md border border-border bg-muted px-2 py-0.5 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          edit
        </button>
      </div>

      <MarkdownContent>{detail.body}</MarkdownContent>

      <div className="mt-3 grid grid-cols-2 gap-x-6 gap-y-1">
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            scope
          </span>
          <span className="font-mono text-xs text-muted-foreground">{detail.scope_path}</span>
        </div>
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            created by
          </span>
          <span className="font-mono text-xs text-muted-foreground">{detail.created_by}</span>
        </div>
        {detail.meta?.confidence && (
          <div className="flex items-baseline gap-2">
            <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
              confidence
            </span>
            <span className="font-mono text-xs text-muted-foreground">
              {detail.meta.confidence}
            </span>
          </div>
        )}
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            updated
          </span>
          <span className="font-mono text-xs text-muted-foreground">
            {new Date(detail.updated_at).toLocaleString()}
          </span>
        </div>
      </div>

      {detail.meta?.tags && detail.meta.tags.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {detail.meta.tags.map((tag) => (
            <span
              key={tag}
              className="inline-flex items-center rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
            >
              {tag}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

export function SnippetCard({
  row,
  highlightMatches = false,
  isExpanded,
  onToggle,
  className,
}: {
  row: EntryRow;
  highlightMatches?: boolean;
  isExpanded?: boolean;
  onToggle?: () => void;
  className?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const isOpen = isExpanded ?? expanded;
  const handleToggle = onToggle ?? (() => setExpanded((p) => !p));
  const articleRef = useRef<HTMLElement>(null);

  return (
    <article
      ref={articleRef}
      className={cn(
        "group rounded-lg border border-border/60 bg-card/80 p-4 transition-all duration-200 hover:border-border/80 hover:bg-accent/20",
        isOpen && "ring-1 ring-ring/15",
        className,
      )}
    >
      <div
        className="cursor-pointer"
        onClick={handleToggle}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            handleToggle();
          }
        }}
      >
        <EntrySummary row={row} highlightMatches={highlightMatches} />
      </div>

      {isOpen && <ExpandedBody entryId={row.id} containerRef={articleRef} />}
    </article>
  );
}
