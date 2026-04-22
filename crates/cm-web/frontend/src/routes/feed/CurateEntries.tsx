import type { RefObject } from "react";
import type { BrowseView } from "@/api/client";
import type { WebBrowseRow } from "@/api/generated/WebBrowseRow";
import { HoistedHeader } from "@/components/composed/HoistedHeader";
import { EntryCard } from "@/components/EntryCard";

interface CurateEntriesProps {
  entries: WebBrowseRow[];
  browseHeader?: BrowseView["header"];
  expandedIds: Set<string>;
  highlightedId: string | null;
  mergeMode: boolean;
  selectedIds: Set<string>;
  isFetchingNextPage: boolean;
  sentinelRef: RefObject<HTMLDivElement | null>;
  setEntryRef: (id: string) => (node: HTMLDivElement | null) => void;
  onToggleExpanded: (id: string) => void;
  onToggleSelection: (id: string) => void;
}

export function CurateEntries({
  entries,
  browseHeader,
  expandedIds,
  highlightedId,
  mergeMode,
  selectedIds,
  isFetchingNextPage,
  sentinelRef,
  setEntryRef,
  onToggleExpanded,
  onToggleSelection,
}: CurateEntriesProps) {
  return (
    <div className="space-y-2">
      {browseHeader && (
        <HoistedHeader
          scope={browseHeader.scope}
          kind={browseHeader.kind}
          createdBy={browseHeader.created_by}
        />
      )}
      {entries.map((row) => (
        <div key={row.id} ref={setEntryRef(row.id)} className="flex items-start gap-2 scroll-mt-16">
          {mergeMode && (
            <button
              type="button"
              onClick={() => onToggleSelection(row.id)}
              className={`mt-4 flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors ${
                selectedIds.has(row.id)
                  ? "border-ring bg-foreground text-background"
                  : "border-border bg-card hover:border-ring/50"
              }`}
              aria-label={`Select ${row.title} for merge`}
            >
              {selectedIds.has(row.id) && (
                <svg className="h-3 w-3" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                  <path
                    d="M2.5 6L5 8.5L9.5 3.5"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              )}
            </button>
          )}
          <div className="min-w-0 flex-1">
            <EntryCard
              row={row}
              isExpanded={!mergeMode && expandedIds.has(row.id)}
              className={entryCardClassName(row.id, expandedIds, highlightedId)}
              onToggle={
                mergeMode ? () => onToggleSelection(row.id) : () => onToggleExpanded(row.id)
              }
            />
          </div>
        </div>
      ))}

      <div ref={sentinelRef} className="h-1" />

      {isFetchingNextPage && (
        <div className="py-4 text-center">
          <p className="text-sm text-muted-foreground">Loading more...</p>
        </div>
      )}
    </div>
  );
}

function entryCardClassName(
  id: string,
  expandedIds: Set<string>,
  highlightedId: string | null,
): string | undefined {
  if (!expandedIds.has(id)) return undefined;
  if (highlightedId === id) {
    return "border-amber-400/40 bg-amber-500/8 ring-2 ring-amber-300/30 transition-all duration-500";
  }
  return "border-border/90 bg-accent/10 ring-1 ring-ring/25 transition-all duration-300";
}
