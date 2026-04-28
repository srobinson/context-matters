import { useCallback, useState } from "react";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useAgentBrowse } from "@/api/hooks";
import { HoistedHeader } from "./composed/HoistedHeader";
import { FilterBar, type FilterState } from "./FilterBar";
import { SnippetCard } from "./SnippetCard";
import { TracePanel } from "./TracePanel";

interface BrowsePaneProps {
  expandedIds: Set<string>;
  onToggleExpanded: (id: string) => void;
}

/**
 * Browse pane for Feed. Owns its filter state, cursor, and fetch
 * via `useAgentBrowse`, then renders the projection view header in a
 * collapsible TracePanel above a list of SnippetCards. Parent passes
 * only the cross-pane expansion state.
 */
export function BrowsePane({ expandedIds, onToggleExpanded }: BrowsePaneProps) {
  const [scope, setScope] = useState<string | undefined>(undefined);
  const [kind, setKind] = useState<EntryKind | undefined>(undefined);
  const [tag, setTag] = useState<string | undefined>(undefined);
  const [agent, setAgent] = useState<string | undefined>(undefined);
  const [forgotten, setForgotten] = useState(false);
  const [cursor, setCursor] = useState<string | undefined>(undefined);

  const query = useAgentBrowse({
    scope,
    kind,
    tag,
    created_by: agent,
    include_superseded: forgotten || undefined,
    limit: 20,
    cursor,
  });

  const handleFilterChange = useCallback((update: Partial<FilterState>) => {
    if ("scope" in update) setScope(update.scope);
    if ("kind" in update) setKind(update.kind);
    if ("tag" in update) setTag(update.tag);
    if ("created_by" in update) setAgent(update.created_by);
    if ("show_forgotten" in update) setForgotten(!!update.show_forgotten);
    setCursor(undefined);
  }, []);

  const handleNext = useCallback(() => {
    const next = query.data?.next_cursor;
    if (next) setCursor(next);
  }, [query.data?.next_cursor]);

  const handleReset = useCallback(() => setCursor(undefined), []);

  return (
    <>
      <FilterBar
        filters={{
          scope,
          kind,
          tag,
          created_by: agent,
          show_forgotten: forgotten,
        }}
        onChange={handleFilterChange}
      />

      {query.isLoading && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">Browsing...</p>
        </div>
      )}

      {query.isError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            Browse failed: {query.error?.message ?? "Unknown error"}
          </p>
        </div>
      )}

      {query.data && (
        <>
          <HoistedHeader
            scope={query.data.header.scope}
            kind={query.data.header.kind}
            createdBy={query.data.header.created_by}
          />
          <TracePanel
            data={{
              kind: "browse",
              header: query.data.header,
              has_more: query.data.has_more,
            }}
          />
          {query.data.entries.length === 0 ? (
            <div className="rounded-lg border border-border bg-card p-8 text-center">
              <p className="text-sm text-muted-foreground">No entries match the current filters.</p>
            </div>
          ) : (
            <div className="space-y-2">
              {query.data.entries.map((row) => (
                <SnippetCard
                  key={row.id}
                  row={row}
                  isExpanded={expandedIds.has(row.id)}
                  onToggle={() => onToggleExpanded(row.id)}
                />
              ))}
            </div>
          )}

          <div className="flex items-center justify-between">
            {cursor ? (
              <button
                type="button"
                onClick={handleReset}
                className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                first page
              </button>
            ) : (
              <div />
            )}
            {query.data.has_more && query.data.next_cursor && (
              <button
                type="button"
                onClick={handleNext}
                className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                next page
              </button>
            )}
          </div>
        </>
      )}
    </>
  );
}
