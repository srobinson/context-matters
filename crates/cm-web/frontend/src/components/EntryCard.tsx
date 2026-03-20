import { useCallback, useState } from "react";
import { toast } from "sonner";
import type { Entry } from "@/api/generated/Entry";
import type { EntryRelation } from "@/api/generated/EntryRelation";
import { useEntry, useForgetEntry } from "@/api/hooks";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { cn } from "@/lib/utils";
import { EntrySummary } from "./composed/EntrySummary";
import { MarkdownContent } from "./composed/MarkdownContent";
import { KindBadge } from "./domain/KindBadge";
import { getQualityIssues, QualityBadge } from "./domain/QualityBadge";
import { EntryEditor } from "./EntryEditor";
import { MutationHistory } from "./MutationHistory";

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
            <span className="rounded bg-muted px-1 py-0.5 text-[10px]">{rel.relation}</span>
            <span className="truncate">{targetId}</span>
          </div>
        );
      })}
    </div>
  );
}

function QualityBadges({ entry }: { entry: Entry }) {
  const issues = getQualityIssues(entry);
  if (issues.length === 0) return null;
  return (
    <div className="flex gap-1">
      {issues.map((issue) => (
        <QualityBadge key={issue} issue={issue} />
      ))}
    </div>
  );
}

function ExpandedContent({ entryId, onForgotten }: { entryId: string; onForgotten?: () => void }) {
  const { data: detail, isLoading, refetch } = useEntry(entryId);
  const [isEditing, setIsEditing] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const forgetEntry = useForgetEntry();

  const handleEditSaved = useCallback(() => {
    setIsEditing(false);
    refetch();
  }, [refetch]);

  const handleForget = useCallback(() => {
    if (!detail) return;
    forgetEntry.mutate(detail.id, {
      onSuccess: () => {
        toast.success(`Forgotten "${detail.title}"`);
        onForgotten?.();
      },
    });
  }, [detail, forgetEntry, onForgotten]);

  if (isLoading || !detail) {
    return <div className="pt-3 text-xs text-muted-foreground">Loading...</div>;
  }

  if (isEditing) {
    return (
      <EntryEditor entry={detail} onCancel={() => setIsEditing(false)} onSaved={handleEditSaved} />
    );
  }

  const meta = detail.meta;
  const allTags = meta?.tags ?? [];

  return (
    <div className="space-y-4 border-t border-border pt-3" onClick={(e) => e.stopPropagation()}>
      {/* Full markdown body */}
      <MarkdownContent>{detail.body}</MarkdownContent>

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

      {/* Quality badges */}
      <QualityBadges entry={detail} />

      {/* Metadata grid */}
      <div className="grid grid-cols-2 gap-x-6 gap-y-1.5">
        <MetadataRow label="kind" value={<KindBadge kind={detail.kind} />} />
        <MetadataRow label="scope" value={detail.scope_path} />
        <MetadataRow label="confidence" value={meta?.confidence ?? "none"} />
        <MetadataRow label="created by" value={detail.created_by} />
        <MetadataRow label="created" value={new Date(detail.created_at).toLocaleString()} />
        <MetadataRow label="updated" value={new Date(detail.updated_at).toLocaleString()} />
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
          <MetadataRow label="expires" value={new Date(meta.expires_at).toLocaleString()} />
        )}
        {meta?.priority != null && <MetadataRow label="priority" value={meta.priority} />}
      </div>

      {/* Relations */}
      {(detail.relations_from.length > 0 || detail.relations_to.length > 0) && (
        <div className="space-y-2">
          <RelationsList relations={detail.relations_from} direction="from" />
          <RelationsList relations={detail.relations_to} direction="to" />
        </div>
      )}

      {/* Mutation history */}
      {showHistory && <MutationHistory entryId={detail.id} />}

      {/* Action buttons */}
      <div className="flex gap-2 pt-1">
        <button
          type="button"
          onClick={() => setIsEditing(true)}
          className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          edit
        </button>
        <button
          type="button"
          onClick={() => setShowHistory((prev) => !prev)}
          className={`rounded-md border px-3 py-1.5 font-mono text-xs transition-colors ${
            showHistory
              ? "border-ring bg-accent text-foreground"
              : "border-border bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
          }`}
        >
          history
        </button>
        <AlertDialog>
          <AlertDialogTrigger
            render={
              <button
                type="button"
                className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-1.5 font-mono text-xs text-destructive transition-colors hover:bg-destructive/10 disabled:opacity-50"
              />
            }
            disabled={forgetEntry.isPending}
          >
            {forgetEntry.isPending ? "forgetting..." : "forget"}
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle className="font-mono text-sm">Forget this entry?</AlertDialogTitle>
              <AlertDialogDescription className="font-mono text-xs">
                &ldquo;{detail.title}&rdquo; will be marked as forgotten. It will no longer appear
                in recall results but can still be viewed with the &ldquo;show forgotten&rdquo;
                filter.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel className="font-mono text-xs">cancel</AlertDialogCancel>
              <AlertDialogAction
                onClick={handleForget}
                className="bg-destructive text-destructive-foreground hover:bg-destructive/90 font-mono text-xs"
              >
                forget
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
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
  const isForgotten = entry.superseded_by != null;

  return (
    <article
      className={cn(
        "group rounded-lg border border-border bg-card p-4 transition-all duration-200 hover:border-border/80 hover:bg-accent/30 dark:hover:bg-accent/20",
        isForgotten && "opacity-40",
        isExpanded && "ring-1 ring-ring/20 dark:ring-ring/30",
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
        <div className="min-w-0 flex-1">
          <EntrySummary entry={entry} showQuality={!isExpanded} />
        </div>
      </div>

      {/* Expanded content */}
      {isExpanded && <ExpandedContent entryId={entry.id} onForgotten={onToggle} />}
    </article>
  );
}
