import { useCallback, useState } from "react";
import { toast } from "sonner";
import type { Entry } from "@/api/generated/Entry";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { NewEntry } from "@/api/generated/NewEntry";
import { useForgetEntry, useMergeEntry } from "@/api/hooks";
import { timeAgo } from "@/lib/time";
import { cn } from "@/lib/utils";
import { KindBadge } from "./domain/KindBadge";

interface MergePanelProps {
  entries: Entry[];
  onComplete: () => void;
  onCancel: () => void;
}

function EntryPreview({
  entry,
  isSelected,
  onSelect,
}: {
  entry: Entry;
  isSelected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "w-full rounded-lg border p-3 text-left transition-all",
        isSelected
          ? "border-ring bg-accent/40 ring-1 ring-ring/30"
          : "border-border bg-card hover:border-border/80 hover:bg-accent/20",
      )}
    >
      <div className="flex items-start gap-2">
        <KindBadge kind={entry.kind} />
        <div className="min-w-0 flex-1 space-y-1">
          <p className="truncate text-sm font-medium text-foreground">{entry.title}</p>
          <div className="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground">
            <span>{entry.scope_path}</span>
            <span className="text-muted-foreground/30">/</span>
            <span>{timeAgo(entry.updated_at)}</span>
          </div>
          <p className="line-clamp-2 font-mono text-xs text-muted-foreground/70">
            {entry.body
              .split("\n")
              .filter((l) => l.trim())
              .slice(0, 2)
              .join(" ")}
          </p>
          {(entry.meta?.tags ?? []).length > 0 && (
            <div className="flex flex-wrap gap-1">
              {(entry.meta?.tags ?? []).slice(0, 5).map((tag) => (
                <span
                  key={tag}
                  className="rounded-md bg-muted px-1 py-0.5 font-mono text-[9px] text-muted-foreground"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
      {isSelected && (
        <span className="mt-1.5 block font-mono text-[10px] text-foreground/60">
          base entry (content will be kept)
        </span>
      )}
    </button>
  );
}

export function MergePanel({ entries, onComplete, onCancel }: MergePanelProps) {
  const [baseIndex, setBaseIndex] = useState(0);
  const mergeEntry = useMergeEntry();
  const forgetEntry = useForgetEntry();

  const baseEntry = entries[baseIndex];
  const othersToSupersede = entries.filter((_, i) => i !== baseIndex);

  const handleMerge = useCallback(async () => {
    if (!baseEntry) return;

    const tags = new Set<string>();
    for (const e of entries) {
      for (const t of e.meta?.tags ?? []) {
        tags.add(t);
      }
    }

    const newEntry: NewEntry = {
      scope_path: baseEntry.scope_path,
      kind: baseEntry.kind as EntryKind,
      title: baseEntry.title,
      body: baseEntry.body,
      created_by: baseEntry.created_by,
      meta: {
        ...baseEntry.meta,
        tags: [...tags],
      },
    };

    try {
      // Supersede the first "other" entry into the merged replacement
      const firstOther = othersToSupersede[0];
      if (!firstOther) return;

      await mergeEntry.mutateAsync({
        oldId: firstOther.id,
        newEntry,
      });

      // Forget remaining others
      for (const other of othersToSupersede.slice(1)) {
        await forgetEntry.mutateAsync(other.id);
      }

      toast.success(`Merged ${entries.length} entries, kept "${baseEntry.title}"`);
      onComplete();
    } catch {
      toast.error("Merge failed");
    }
  }, [baseEntry, entries, othersToSupersede, mergeEntry, forgetEntry, onComplete]);

  const isPending = mergeEntry.isPending || forgetEntry.isPending;

  return (
    <div className="space-y-4 rounded-lg border border-border bg-card p-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-foreground">Merge {entries.length} entries</h3>
          <p className="font-mono text-[10px] text-muted-foreground">
            Select the base entry to keep. Others will be superseded. Tags are combined.
          </p>
        </div>
        <button
          type="button"
          onClick={onCancel}
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          cancel
        </button>
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        {entries.map((entry, i) => (
          <EntryPreview
            key={entry.id}
            entry={entry}
            isSelected={i === baseIndex}
            onSelect={() => setBaseIndex(i)}
          />
        ))}
      </div>

      <div className="flex items-center gap-3 border-t border-border pt-3">
        <button
          type="button"
          onClick={handleMerge}
          disabled={isPending || entries.length < 2}
          className="rounded-md bg-foreground px-3 py-1.5 font-mono text-xs text-background transition-colors hover:bg-foreground/90 disabled:opacity-50"
        >
          {isPending ? "merging..." : `merge into "${baseEntry?.title}"`}
        </button>
        <span className="font-mono text-[10px] text-muted-foreground">
          {othersToSupersede.length} {othersToSupersede.length === 1 ? "entry" : "entries"} will be
          superseded
        </span>
      </div>
    </div>
  );
}
