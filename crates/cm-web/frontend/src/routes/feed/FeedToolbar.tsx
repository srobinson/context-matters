import { GitMerge, Plus } from "lucide-react";
import type { BrowseSort } from "@/api/generated/BrowseSort";
import { type FeedMode, FeedModeSelect } from "@/components/domain/FeedModeSelect";
import { SortSelect } from "@/components/domain/SortSelect";

interface FeedToolbarProps {
  activeMode: FeedMode;
  sort: BrowseSort;
  entriesCount: number;
  totalCount: number;
  mergeMode: boolean;
  showNewEntry: boolean;
  isBrowseMode: boolean;
  onModeChange: (mode: FeedMode) => void;
  onSortChange: (sort: BrowseSort) => void;
  onToggleMergeMode: () => void;
  onShowNewEntry: () => void;
}

export function FeedToolbar({
  activeMode,
  sort,
  entriesCount,
  totalCount,
  mergeMode,
  showNewEntry,
  isBrowseMode,
  onModeChange,
  onSortChange,
  onToggleMergeMode,
  onShowNewEntry,
}: FeedToolbarProps) {
  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-3">
        <h2 className="text-lg font-medium tracking-tight">Feed</h2>
        <FeedModeSelect value={activeMode} onChange={onModeChange} />
        {activeMode === "curate" && <SortSelect value={sort} onChange={onSortChange} />}
      </div>
      <div className="flex items-center gap-3">
        {activeMode === "curate" && entriesCount > 0 && (
          <span className="font-mono text-xs text-muted-foreground">
            {entriesCount}
            {totalCount > entriesCount ? ` / ${totalCount}` : ""} entries
          </span>
        )}
        <button
          type="button"
          onClick={onToggleMergeMode}
          disabled={activeMode !== "curate"}
          className={`flex items-center gap-1 rounded-md border px-2 py-1 font-mono text-xs transition-colors ${
            mergeMode
              ? "border-ring bg-accent text-foreground"
              : "border-border bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
          }`}
        >
          <GitMerge className="h-3 w-3" />
          {activeMode !== "curate" ? "merge unavailable" : mergeMode ? "cancel merge" : "merge"}
        </button>
        <button
          type="button"
          onClick={onShowNewEntry}
          disabled={showNewEntry || mergeMode || isBrowseMode}
          className="flex items-center gap-1 rounded-md border border-border bg-muted px-2 py-1 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
        >
          <Plus className="h-3 w-3" />
          new
        </button>
      </div>
    </div>
  );
}
