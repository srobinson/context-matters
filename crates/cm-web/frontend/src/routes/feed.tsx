import { useCallback, useMemo, useState } from "react";
import { createRoute, useNavigate } from "@tanstack/react-router";
import { rootRoute } from "./__root";
import type { BrowseSort } from "@/api/generated/BrowseSort";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useEntries } from "@/api/hooks";
import { EntryCard } from "@/components/EntryCard";
import { SortSelect } from "@/components/SortSelect";
import { useIntersectionObserver } from "@/hooks/useIntersectionObserver";

export type FeedSearch = {
  scope_path?: string;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  sort?: BrowseSort;
  show_forgotten?: boolean;
};

export const feedRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/feed",
  validateSearch: (search: Record<string, unknown>): FeedSearch => ({
    scope_path:
      typeof search["scope_path"] === "string"
        ? search["scope_path"]
        : undefined,
    kind: isEntryKind(search["kind"]) ? search["kind"] : undefined,
    tag: typeof search["tag"] === "string" ? search["tag"] : undefined,
    created_by:
      typeof search["created_by"] === "string"
        ? search["created_by"]
        : undefined,
    sort: isBrowseSort(search["sort"]) ? search["sort"] : undefined,
    show_forgotten:
      search["show_forgotten"] === true ||
      search["show_forgotten"] === "true",
  }),
  component: FeedPage,
});

function FeedPage() {
  const { sort, kind, scope_path, tag, created_by, show_forgotten } =
    feedRoute.useSearch();

  const navigate = useNavigate({ from: "/feed" });
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const handleSortChange = useCallback(
    (newSort: BrowseSort) => {
      navigate({
        search: (prev) => ({
          ...prev,
          sort: newSort === "recent" ? undefined : newSort,
        }),
      });
    },
    [navigate],
  );

  const {
    data,
    isLoading,
    isError,
    error,
    hasNextPage,
    fetchNextPage,
    isFetchingNextPage,
  } = useEntries({
    sort: sort ?? "recent",
    kind,
    scope_path,
    tag,
    created_by,
    include_superseded: show_forgotten,
    limit: 30,
  });

  const handleLoadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const sentinelRef = useIntersectionObserver(
    handleLoadMore,
    !!hasNextPage && !isFetchingNextPage,
  );

  const entries = useMemo(
    () => data?.pages.flatMap((page) => page.items) ?? [],
    [data],
  );

  const totalCount = data?.pages[0]?.total ?? 0;

  const activeFilters: string[] = [
    kind ? `kind:${kind}` : "",
    scope_path ? `scope:${scope_path}` : "",
    tag ? `tag:${tag}` : "",
    created_by ? `by:${created_by}` : "",
    show_forgotten ? "show:forgotten" : "",
  ].filter((s) => s.length > 0);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-medium tracking-tight">Feed</h2>
          <SortSelect
            value={sort ?? "recent"}
            onChange={handleSortChange}
          />
        </div>
        {entries.length > 0 && (
          <span className="font-mono text-xs text-muted-foreground">
            {entries.length}
            {totalCount > entries.length && ` / ${totalCount}`}
            {" entries"}
          </span>
        )}
      </div>

      {activeFilters.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {activeFilters.map((f) => (
            <span
              key={f}
              className="inline-flex items-center rounded-md border border-border bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground"
            >
              {f}
            </span>
          ))}
        </div>
      )}

      {isLoading && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">Loading entries...</p>
        </div>
      )}

      {isError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            Failed to load entries: {error.message}
          </p>
        </div>
      )}

      {!isLoading && entries.length === 0 && !isError && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">No entries found.</p>
        </div>
      )}

      {entries.length > 0 && (
        <div className="space-y-2">
          {entries.map((entry) => (
            <EntryCard
              key={entry.id}
              entry={entry}
              isExpanded={expandedId === entry.id}
              onToggle={() =>
                setExpandedId((prev) =>
                  prev === entry.id ? null : entry.id,
                )
              }
            />
          ))}

          {/* Infinite scroll sentinel */}
          <div ref={sentinelRef} className="h-1" />

          {isFetchingNextPage && (
            <div className="py-4 text-center">
              <p className="text-sm text-muted-foreground">
                Loading more...
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

const ENTRY_KINDS: ReadonlySet<string> = new Set([
  "fact",
  "decision",
  "preference",
  "lesson",
  "reference",
  "feedback",
  "pattern",
  "observation",
]);

const BROWSE_SORTS: ReadonlySet<string> = new Set([
  "recent",
  "oldest",
  "title_asc",
  "title_desc",
  "scope_asc",
  "scope_desc",
  "kind_asc",
  "kind_desc",
]);

function isEntryKind(v: unknown): v is EntryKind {
  return typeof v === "string" && ENTRY_KINDS.has(v);
}

function isBrowseSort(v: unknown): v is BrowseSort {
  return typeof v === "string" && BROWSE_SORTS.has(v);
}
