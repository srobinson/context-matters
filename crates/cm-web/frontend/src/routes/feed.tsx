import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createRoute, useNavigate } from "@tanstack/react-router";
import { rootRoute } from "./__root";
import type { BrowseSort } from "@/api/generated/BrowseSort";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useEntries, useSearch } from "@/api/hooks";
import { EntryCard } from "@/components/EntryCard";
import { FilterBar, type FilterState } from "@/components/FilterBar";
import { NewEntryEditor } from "@/components/NewEntryEditor";
import { SortSelect } from "@/components/SortSelect";
import { Input } from "@/components/ui/input";
import { useDebounce } from "@/hooks/useDebounce";
import { useIntersectionObserver } from "@/hooks/useIntersectionObserver";
import { Plus, Search, X } from "lucide-react";

export type FeedSearch = {
  scope_path?: string;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  sort?: BrowseSort;
  show_forgotten?: boolean;
  q?: string;
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
    q: typeof search["q"] === "string" && search["q"] ? search["q"] : undefined,
  }),
  component: FeedPage,
});

function FeedPage() {
  const { sort, kind, scope_path, tag, created_by, show_forgotten, q } =
    feedRoute.useSearch();

  const navigate = useNavigate({ from: "/feed" });
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showNewEntry, setShowNewEntry] = useState(false);
  const [searchInput, setSearchInput] = useState(q ?? "");
  const inputRef = useRef<HTMLInputElement>(null);
  const debouncedQuery = useDebounce(searchInput, 300);

  const isSearching = !!debouncedQuery;

  // Sync debounced query to URL
  useEffect(() => {
    const urlQ = q ?? "";
    if (debouncedQuery !== urlQ) {
      navigate({
        search: (prev) => ({
          ...prev,
          q: debouncedQuery || undefined,
        }),
        replace: true,
      });
    }
  }, [debouncedQuery, q, navigate]);

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

  const handleFilterChange = useCallback(
    (update: Partial<FilterState>) => {
      navigate({
        search: (prev) => ({ ...prev, ...update }),
      });
    },
    [navigate],
  );

  const handleClearSearch = useCallback(() => {
    setSearchInput("");
    inputRef.current?.focus();
  }, []);

  // Browse query (used when not searching)
  const browseQuery = useEntries({
    sort: sort ?? "recent",
    kind,
    scope_path,
    tag,
    created_by,
    include_superseded: show_forgotten,
    limit: 30,
  });

  // Search query (used when searching)
  const searchQuery = useSearch({
    query: debouncedQuery ?? "",
    scope_path,
    kind,
    tag,
    limit: 30,
  });

  const activeQuery = isSearching ? searchQuery : browseQuery;

  const {
    data,
    isLoading,
    isError,
    error,
    hasNextPage,
    fetchNextPage,
    isFetchingNextPage,
  } = activeQuery;

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

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-medium tracking-tight">Feed</h2>
          {!isSearching && (
            <SortSelect
              value={sort ?? "recent"}
              onChange={handleSortChange}
            />
          )}
        </div>
        <div className="flex items-center gap-3">
          {entries.length > 0 && (
            <span className="font-mono text-xs text-muted-foreground">
              {entries.length}
              {totalCount > entries.length && ` / ${totalCount}`}
              {isSearching ? " results" : " entries"}
            </span>
          )}
          <button
            type="button"
            onClick={() => setShowNewEntry(true)}
            disabled={showNewEntry}
            className="flex items-center gap-1 rounded-md border border-border bg-muted px-2 py-1 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
          >
            <Plus className="h-3 w-3" />
            new
          </button>
        </div>
      </div>

      <div className="relative">
        <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          ref={inputRef}
          type="text"
          placeholder="Search entries (FTS5)..."
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          className="pl-8 pr-8 font-mono text-xs"
        />
        {searchInput && (
          <button
            type="button"
            onClick={handleClearSearch}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      {!isSearching && (
        <FilterBar
          filters={{ scope_path, kind, tag, created_by, show_forgotten }}
          onChange={handleFilterChange}
        />
      )}

      {isSearching && (scope_path || kind || tag) && (
        <div className="flex items-center gap-1.5 font-mono text-xs text-muted-foreground">
          <span>Searching within:</span>
          {scope_path && <span className="rounded-md border border-border bg-muted px-1.5 py-0.5">scope:{scope_path}</span>}
          {kind && <span className="rounded-md border border-border bg-muted px-1.5 py-0.5">kind:{kind}</span>}
          {tag && <span className="rounded-md border border-border bg-muted px-1.5 py-0.5">tag:{tag}</span>}
        </div>
      )}

      {isLoading && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">
            {isSearching ? "Searching..." : "Loading entries..."}
          </p>
        </div>
      )}

      {isError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            {isSearching ? "Search failed" : "Failed to load entries"}:{" "}
            {error.message}
          </p>
        </div>
      )}

      {showNewEntry && (
        <NewEntryEditor
          onCancel={() => setShowNewEntry(false)}
          onCreated={() => setShowNewEntry(false)}
        />
      )}

      {!isLoading && entries.length === 0 && !isError && !showNewEntry && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">
            {isSearching
              ? `No results for "${debouncedQuery}".`
              : "No entries found."}
          </p>
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
