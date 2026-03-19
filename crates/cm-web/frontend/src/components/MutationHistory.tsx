import { useState, useMemo } from "react";
import type { MutationRecord } from "@/api/generated/MutationRecord";
import type { MutationSource } from "@/api/generated/MutationSource";
import type { MutationAction } from "@/api/generated/MutationAction";
import { useMutationHistory } from "@/api/hooks";
import { timeAgo } from "@/lib/time";

const ALL_SOURCES: MutationSource[] = ["mcp", "cli", "web", "helix"];

const ACTION_STYLES: Record<MutationAction, string> = {
  create: "text-green-600 dark:text-green-400",
  update: "text-blue-600 dark:text-blue-400",
  forget: "text-red-600 dark:text-red-400",
  supersede: "text-orange-600 dark:text-orange-400",
};

function summarizeChanges(record: MutationRecord): string | null {
  if (record.action === "create") return "entry created";
  if (record.action === "forget") return "entry forgotten";
  if (record.action === "supersede") return "superseded by newer entry";

  const before = record.before_snapshot as Record<string, unknown> | null;
  const after = record.after_snapshot as Record<string, unknown> | null;
  if (!before || !after) return null;

  const changed: string[] = [];
  for (const key of ["title", "body", "kind", "scope_path"]) {
    if (JSON.stringify(before[key]) !== JSON.stringify(after[key])) {
      changed.push(key);
    }
  }

  const beforeMeta = before.meta as Record<string, unknown> | null;
  const afterMeta = after.meta as Record<string, unknown> | null;
  if (JSON.stringify(beforeMeta?.tags) !== JSON.stringify(afterMeta?.tags)) {
    changed.push("tags");
  }
  if (beforeMeta?.confidence !== afterMeta?.confidence) {
    changed.push("confidence");
  }

  if (changed.length === 0) return "metadata updated";
  return changed.join(", ");
}

export function MutationHistory({ entryId }: { entryId: string }) {
  const [source, setSource] = useState<string | undefined>(undefined);

  const { data, isLoading } = useMutationHistory({
    entry_id: entryId,
    source,
    limit: 50,
  });

  const records = useMemo(
    () => data?.items ?? [],
    [data],
  );

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          mutation history
        </span>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setSource(undefined)}
            className={`rounded-md px-1.5 py-0.5 font-mono text-[10px] transition-colors ${
              source === undefined
                ? "bg-foreground text-background"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            all
          </button>
          {ALL_SOURCES.map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setSource(s)}
              className={`rounded-md px-1.5 py-0.5 font-mono text-[10px] transition-colors ${
                source === s
                  ? "bg-foreground text-background"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {s}
            </button>
          ))}
        </div>
      </div>

      {isLoading && (
        <div className="space-y-2">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="animate-pulse rounded-md border border-border bg-muted/30 h-10"
            />
          ))}
        </div>
      )}

      {!isLoading && records.length === 0 && (
        <p className="rounded-md border border-border bg-muted/20 p-3 text-center font-mono text-xs text-muted-foreground">
          No mutations{source ? ` from ${source}` : ""} recorded for this entry.
        </p>
      )}

      {records.length > 0 && (
        <div className="space-y-1">
          {records.map((record) => (
            <MutationRow key={record.id} record={record} />
          ))}
        </div>
      )}
    </div>
  );
}

function MutationRow({ record }: { record: MutationRecord }) {
  const summary = summarizeChanges(record);

  return (
    <div className="flex items-start gap-2.5 rounded-md border border-border bg-muted/20 px-2.5 py-2">
      <span
        className={`shrink-0 font-mono text-[10px] font-medium ${ACTION_STYLES[record.action]}`}
      >
        {record.action}
      </span>
      <div className="min-w-0 flex-1">
        {summary && (
          <p className="font-mono text-xs text-muted-foreground">{summary}</p>
        )}
        <div className="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground/60">
          <span className="rounded bg-muted px-1 py-0.5">{record.source}</span>
          <time
            dateTime={record.timestamp}
            title={new Date(record.timestamp).toLocaleString()}
          >
            {timeAgo(record.timestamp)}
          </time>
        </div>
      </div>
    </div>
  );
}
