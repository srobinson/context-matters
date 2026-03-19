import Markdown from "react-markdown";
import type { Entry } from "@/api/generated/Entry";
import type { EntryRelation } from "@/api/generated/EntryRelation";
import { useEntry } from "@/api/hooks";
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

function MetadataRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-2">
      <span className="shrink-0 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        {label}
      </span>
      <span className="font-mono text-xs text-muted-foreground">{value}</span>
    </div>
  );
}

function RelationsList({
  relations,
  direction,
}: {
  relations: EntryRelation[];
  direction: "from" | "to";
}) {
  if (relations.length === 0) return null;
  return (
    <div className="space-y-1">
      <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        {direction === "from" ? "relates to" : "related from"}
      </span>
      {relations.map((rel) => {
        const targetId = direction === "from" ? rel.target_id : rel.source_id;
        return (
          <div
            key={`${rel.source_id}-${rel.target_id}-${rel.relation}`}
            className="flex items-center gap-2 font-mono text-xs text-muted-foreground"
          >
            <span className="rounded bg-muted px-1 py-0.5 text-[10px]">
              {rel.relation}
            </span>
            <span className="truncate">{targetId}</span>
          </div>
        );
      })}
    </div>
  );
}

function ExpandedContent({ entryId }: { entryId: string }) {
  const { data: detail, isLoading } = useEntry(entryId);

  if (isLoading || !detail) {
    return (
      <div className="pt-3 text-xs text-muted-foreground">Loading...</div>
    );
  }

  const meta = detail.meta;
  const allTags = meta?.tags ?? [];

  return (
    <div
      className="space-y-4 border-t border-border pt-3"
      onClick={(e) => e.stopPropagation()}
    >
      {/* Full markdown body */}
      <div className="prose prose-sm prose-neutral dark:prose-invert max-w-none font-mono text-xs leading-relaxed [&_pre]:bg-muted [&_pre]:p-3 [&_pre]:rounded-md [&_code]:text-[11px] [&_h1]:text-sm [&_h2]:text-sm [&_h3]:text-xs [&_p]:text-xs [&_li]:text-xs [&_a]:text-muted-foreground [&_a]:underline">
        <Markdown>{detail.body}</Markdown>
      </div>

      {/* All tags */}
      {allTags.length > 0 && (
        <div className="space-y-1">
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            tags
          </span>
          <div className="flex flex-wrap gap-1">
            {allTags.map((tag) => (
              <span
                key={tag}
                className="inline-flex items-center rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Metadata grid */}
      <div className="grid grid-cols-2 gap-x-6 gap-y-1.5">
        <MetadataRow label="kind" value={detail.kind} />
        <MetadataRow label="scope" value={detail.scope_path} />
        <MetadataRow
          label="confidence"
          value={meta?.confidence ?? "none"}
        />
        <MetadataRow label="created by" value={detail.created_by} />
        <MetadataRow
          label="created"
          value={new Date(detail.created_at).toLocaleString()}
        />
        <MetadataRow
          label="updated"
          value={new Date(detail.updated_at).toLocaleString()}
        />
        <MetadataRow
          label="hash"
          value={
            <span className="select-all" title={detail.content_hash}>
              {detail.content_hash.slice(0, 12)}...
            </span>
          }
        />
        {meta?.source && <MetadataRow label="source" value={meta.source} />}
        {meta?.expires_at && (
          <MetadataRow
            label="expires"
            value={new Date(meta.expires_at).toLocaleString()}
          />
        )}
        {meta?.priority != null && (
          <MetadataRow label="priority" value={meta.priority} />
        )}
      </div>

      {/* Relations */}
      {(detail.relations_from.length > 0 ||
        detail.relations_to.length > 0) && (
        <div className="space-y-2">
          <RelationsList
            relations={detail.relations_from}
            direction="from"
          />
          <RelationsList
            relations={detail.relations_to}
            direction="to"
          />
        </div>
      )}

      {/* Action buttons (stubs for ALP-1530 curation) */}
      <div className="flex gap-2 pt-1">
        <button
          type="button"
          className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          edit
        </button>
        <button
          type="button"
          className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-1.5 font-mono text-xs text-destructive transition-colors hover:bg-destructive/10"
        >
          forget
        </button>
      </div>
    </div>
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
    >
      <div
        className="flex items-start gap-3 cursor-pointer"
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

          {/* Row 3: content preview (compact only) */}
          {!isExpanded && <ContentPreview body={entry.body} />}

          {/* Row 4: tags + quality badges (compact only) */}
          {!isExpanded && (tags.length > 0 || qualityIssues.length > 0) && (
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

      {/* Expanded content */}
      {isExpanded && <ExpandedContent entryId={entry.id} />}
    </article>
  );
}
