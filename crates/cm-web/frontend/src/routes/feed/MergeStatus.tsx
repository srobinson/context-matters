import type { EntryDetail } from "@/api/client";
import { MergePanel } from "@/components/MergePanel";

interface MergeStatusProps {
  selectedCount: number;
  selectionHydrated: boolean;
  hydratedSelectedEntries: EntryDetail[];
  onComplete: () => void;
  onCancel: () => void;
}

export function MergeStatus({
  selectedCount,
  selectionHydrated,
  hydratedSelectedEntries,
  onComplete,
  onCancel,
}: MergeStatusProps) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between rounded-lg border border-border bg-muted/50 px-3 py-2">
        <span className="font-mono text-xs text-muted-foreground">
          {selectedCount === 0 ? "Select entries to merge" : `${selectedCount} selected`}
        </span>
        {selectedCount >= 2 && (
          <span className="font-mono text-[10px] text-muted-foreground/60">merge panel below</span>
        )}
      </div>
      {selectedCount >= 2 &&
        (selectionHydrated ? (
          <MergePanel
            entries={hydratedSelectedEntries}
            onComplete={onComplete}
            onCancel={onCancel}
          />
        ) : (
          <div className="rounded-lg border border-border bg-card p-4 text-center">
            <p className="font-mono text-xs text-muted-foreground">Loading selected entries...</p>
          </div>
        ))}
    </div>
  );
}
