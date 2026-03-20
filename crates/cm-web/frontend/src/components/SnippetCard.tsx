import { useState } from "react";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useEntry } from "@/api/hooks";
import { timeAgo } from "@/lib/time";
import { cn } from "@/lib/utils";
import { MarkdownContent } from "./composed/MarkdownContent";
import { KindBadge } from "./domain/KindBadge";

export interface SnippetEntry {
  id: string;
  scope_path: string;
  kind: EntryKind;
  title: string;
  snippet: string;
  created_by: string;
  updated_at: string;
  tags?: string[];
  confidence?: "high" | "medium" | "low" | null;
}

function getAgentName(createdBy: string) {
  const parts = createdBy.split(":");
  return parts.length > 1 ? parts.slice(1).join(":") : createdBy;
}

function ExpandedBody({ entryId }: { entryId: string }) {
  const { data: detail, isLoading } = useEntry(entryId);

  if (isLoading || !detail) {
    return <div className="pt-3 text-xs text-muted-foreground">Loading...</div>;
  }

  return (
    <div className="border-t border-border/50 pt-3" onClick={(e) => e.stopPropagation()}>
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
  entry,
  isExpanded,
  onToggle,
  className,
}: {
  entry: SnippetEntry;
  isExpanded?: boolean;
  onToggle?: () => void;
  className?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const isOpen = isExpanded ?? expanded;
  const handleToggle = onToggle ?? (() => setExpanded((p) => !p));

  const agentName = getAgentName(entry.created_by);
  const tags = entry.tags ?? [];

  return (
    <article
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
        <div className="flex items-start justify-between gap-3">
          <p className="line-clamp-2 text-sm font-medium leading-snug text-foreground">
            {entry.title}
          </p>
          <time
            dateTime={entry.updated_at}
            className="shrink-0 font-mono text-[10px] text-muted-foreground/70"
            title={new Date(entry.updated_at).toLocaleString()}
          >
            {timeAgo(entry.updated_at)}
          </time>
        </div>

        <p className="mt-1.5 line-clamp-2 font-mono text-xs leading-relaxed text-muted-foreground">
          {entry.snippet}
        </p>

        <div className="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[10px] text-muted-foreground">
          <KindBadge kind={entry.kind} className="shrink-0" />
          <span className="text-muted-foreground/30">/</span>
          <span>{agentName}</span>
          <span className="text-muted-foreground/30">/</span>
          <span className="truncate">{entry.scope_path}</span>
          {tags.length > 0 && (
            <>
              <span className="text-muted-foreground/30">/</span>
              <span className="truncate">
                {tags.slice(0, 2).join(", ")}
                {tags.length > 2 && ` +${tags.length - 2}`}
              </span>
            </>
          )}
          {entry.confidence && (
            <>
              <span className="text-muted-foreground/30">/</span>
              <span>{entry.confidence}</span>
            </>
          )}
        </div>
      </div>

      {isOpen && <ExpandedBody entryId={entry.id} />}
    </article>
  );
}
