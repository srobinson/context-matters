import type { Entry } from "@/api/generated/Entry";
import { KindBadge } from "./KindBadge";
import { QualityBadge, getQualityIssues } from "./QualityBadge";
import { timeAgo } from "@/lib/time";
import { cn } from "@/lib/utils";

const MAX_VISIBLE_TAGS = 5;

function ScopeBreadcrumb({ path }: { path: string }) {
  const segments = path.split("/");
  return (
    <span className="inline-flex items-center gap-0.5 font-mono text-[11px] text-muted-foreground">
      {segments.map((segment, i) => (
        <span key={i} className="inline-flex items-center gap-0.5">
          {i > 0 && (
            <span className="text-muted-foreground/40">/</span>
          )}
          <span className="hover:text-foreground transition-colors">
            {segment}
          </span>
        </span>
      ))}
    </span>
  );
}

function AgentName({ createdBy }: { createdBy: string }) {
  const parts = createdBy.split(":");
  const display = parts.length > 1 ? parts.slice(1).join(":") : createdBy;
  return (
    <span className="font-mono text-[11px] text-muted-foreground">
      {display}
    </span>
  );
}

function TagChips({ tags }: { tags: string[] }) {
  if (tags.length === 0) return null;

  const visible = tags.slice(0, MAX_VISIBLE_TAGS);
  const overflow = tags.length - MAX_VISIBLE_TAGS;

  return (
    <div className="flex flex-wrap gap-1">
      {visible.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
        >
          {tag}
        </span>
      ))}
      {overflow > 0 && (
        <span className="inline-flex items-center rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground/60">
          +{overflow} more
        </span>
      )}
    </div>
  );
}

function ContentPreview({ body }: { body: string }) {
  const lines = body.split("\n").filter((l) => l.trim().length > 0);
  const preview = lines.slice(0, 2).join("\n");
  if (!preview) return null;

  return (
    <p className="line-clamp-2 font-mono text-xs leading-relaxed text-muted-foreground/80">
      {preview}
    </p>
  );
}

export function EntryCard({
  entry,
  isExpanded,
  onToggle,
  className,
}: {
  entry: Entry;
  isExpanded?: boolean;
  onToggle?: () => void;
  className?: string;
}) {
  const qualityIssues = getQualityIssues(entry);
  const tags = entry.meta?.tags ?? [];
  const isForgotten = entry.superseded_by != null;

  return (
    <article
      className={cn(
        "group rounded-lg border border-border bg-card p-4 transition-colors hover:border-border/80 hover:bg-accent/30",
        isForgotten && "opacity-50",
        isExpanded && "ring-1 ring-ring/20",
        className,
      )}
      onClick={onToggle}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle?.();
        }
      }}
    >
      <div className="flex items-start gap-3">
        <div className="flex shrink-0 pt-0.5">
          <KindBadge kind={entry.kind} />
        </div>

        <div className="min-w-0 flex-1 space-y-1.5">
          {/* Row 1: title + timestamp */}
          <div className="flex items-baseline justify-between gap-2">
            <h3 className="truncate text-sm font-medium text-foreground">
              {entry.title}
            </h3>
            <time
              dateTime={entry.updated_at}
              className="shrink-0 font-mono text-[11px] text-muted-foreground"
              title={new Date(entry.updated_at).toLocaleString()}
            >
              {timeAgo(entry.updated_at)}
            </time>
          </div>

          {/* Row 2: scope + agent */}
          <div className="flex items-center gap-2">
            <ScopeBreadcrumb path={entry.scope_path} />
            <span className="text-muted-foreground/30">·</span>
            <AgentName createdBy={entry.created_by} />
          </div>

          {/* Row 3: content preview */}
          <ContentPreview body={entry.body} />

          {/* Row 4: tags + quality badges */}
          {(tags.length > 0 || qualityIssues.length > 0) && (
            <div className="flex items-center gap-2 pt-0.5">
              <TagChips tags={tags} />
              {qualityIssues.length > 0 && (
                <div className="flex gap-1">
                  {qualityIssues.map((issue) => (
                    <QualityBadge key={issue} issue={issue} />
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </article>
  );
}
