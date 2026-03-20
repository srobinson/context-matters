import { createRoute, useNavigate } from "@tanstack/react-router";
import { GitMerge, Plus, Search, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { BrowseSort } from "@/api/generated/BrowseSort";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useAgentBrowse, useAgentRecall, useEntries } from "@/api/hooks";
import { type FeedMode, FeedModeSelect } from "@/components/domain/FeedModeSelect";
import { SortSelect } from "@/components/domain/SortSelect";
import { EntryCard } from "@/components/EntryCard";
import { FilterBar, type FilterState } from "@/components/FilterBar";
import { MergePanel } from "@/components/MergePanel";
import { NewEntryEditor } from "@/components/NewEntryEditor";
import { RecallBar } from "@/components/RecallBar";
import { SnippetCard } from "@/components/SnippetCard";
import { TracePanel } from "@/components/TracePanel";
import { Input } from "@/components/ui/input";
import { useDebounce } from "@/hooks/useDebounce";
import { useIntersectionObserver } from "@/hooks/useIntersectionObserver";
import { rootRoute } from "./__root";

export type FeedSearch = {
  mode?: FeedMode;
  scope_path?: string;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  sort?: BrowseSort;
  show_forgotten?: boolean;
  q?: string;
  entry_id?: string;
};

export const feedRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/feed",
  validateSearch: (search: Record<string, unknown>): FeedSearch => ({
    mode: isFeedMode(search.mode) ? search.mode : undefined,
    scope_path: typeof search.scope_path === "string" ? search.scope_path : undefined,
    kind: isEntryKind(search.kind) ? search.kind : undefined,
    tag: typeof search.tag === "string" ? search.tag : undefined,
    created_by: typeof search.created_by === "string" ? search.created_by : undefined,
    sort: isBrowseSort(search.sort) ? search.sort : undefined,
    show_forgotten: search.show_forgotten === true || search.show_forgotten === "true",
    q: typeof search.q === "string" && search.q ? search.q : undefined,
    entry_id: typeof search.entry_id === "string" && search.entry_id ? search.entry_id : undefined,
  }),
  component: FeedPage,
});

function FeedPage() {
  const { mode, sort, kind, scope_path, tag, created_by, show_forgotten, q, entry_id } =
    feedRoute.useSearch();

  const navigate = useNavigate({ from: "/feed" });
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [highlightedId, setHighlightedId] = useState<string | null>(null);
  const [showNewEntry, setShowNewEntry] = useState(false);
  const [searchInput, setSearchInput] = useState(q ?? "");
  const [recallScope, setRecallScope] = useState<string | undefined>(undefined);
  const [recallKinds, setRecallKinds] = useState<EntryKind[]>([]);
  const [recallTags, setRecallTags] = useState<string[]>([]);
  const [recallLimit, setRecallLimit] = useState(20);
  const [recallMaxTokens, setRecallMaxTokens] = useState<number | undefined>(undefined);
  const [browseScope, setBrowseScope] = useState<string | undefined>(undefined);
  const [browseKind, setBrowseKind] = useState<EntryKind | undefined>(undefined);
  const [browseTag, setBrowseTag] = useState<string | undefined>(undefined);
  const [browseAgent, setBrowseAgent] = useState<string | undefined>(undefined);
  const [browseForgotten, setBrowseForgotten] = useState(false);
  const [browseCursor, setBrowseCursor] = useState<string | undefined>(undefined);
  const inputRef = useRef<HTMLInputElement>(null);
  const entryRefs = useRef(new Map<string, HTMLDivElement>());
  const debouncedQuery = useDebounce(searchInput, 300);
  const [mergeMode, setMergeMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const activeMode: FeedMode = mode ?? "curate";
  const isRecallMode = activeMode === "recall";
  const isBrowseMode = activeMode === "browse";

  useEffect(() => {
    if (entry_id) {
      setExpandedIds((prev) => {
        if (prev.has(entry_id)) return prev;
        const next = new Set(prev);
        next.add(entry_id);
        return next;
      });
    }
  }, [entry_id]);

  useEffect(() => {
    if (!entry_id) {
      setHighlightedId(null);
      return;
    }

    setHighlightedId(entry_id);
    const timeoutId = window.setTimeout(() => {
      setHighlightedId((current) => (current === entry_id ? null : current));
    }, 1800);

    return () => window.clearTimeout(timeoutId);
  }, [entry_id]);

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

  useEffect(() => {
    if (!isRecallMode) {
      setSearchInput("");
      setRecallScope(undefined);
      setRecallKinds([]);
      setRecallTags([]);
      setRecallLimit(20);
      setRecallMaxTokens(undefined);
    }
    if (!isBrowseMode) {
      setBrowseScope(undefined);
      setBrowseKind(undefined);
      setBrowseTag(undefined);
      setBrowseAgent(undefined);
      setBrowseForgotten(false);
      setBrowseCursor(undefined);
    }
    if (activeMode !== "curate" && mergeMode) {
      setMergeMode(false);
      setSelectedIds(new Set());
    }
  }, [activeMode, isRecallMode, isBrowseMode, mergeMode]);

  const handleModeChange = useCallback(
    (nextMode: FeedMode) => {
      navigate({
        search: (prev) => ({
          ...prev,
          mode: nextMode === "curate" ? undefined : nextMode,
          q: nextMode !== "recall" ? undefined : prev.q,
        }),
      });
      if (nextMode !== "recall") {
        setSearchInput("");
      }
    },
    [navigate],
  );

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

  const handleBrowseFilterChange = useCallback((update: Partial<FilterState>) => {
    if ("scope_path" in update) setBrowseScope(update.scope_path);
    if ("kind" in update) setBrowseKind(update.kind);
    if ("tag" in update) setBrowseTag(update.tag);
    if ("created_by" in update) setBrowseAgent(update.created_by);
    if ("show_forgotten" in update) setBrowseForgotten(!!update.show_forgotten);
    setBrowseCursor(undefined);
  }, []);

  const toggleMergeMode = useCallback(() => {
    setMergeMode((prev) => {
      if (prev) setSelectedIds(new Set());
      return !prev;
    });
    setExpandedIds(new Set());
  }, []);

  const toggleSelection = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const handleMergeComplete = useCallback(() => {
    setMergeMode(false);
    setSelectedIds(new Set());
  }, []);

  const toggleExpanded = useCallback(
    (id: string) => {
      setExpandedIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        navigate({
          search: (prevSearch) => ({
            ...prevSearch,
            entry_id: next.size === 1 ? [...next][0] : undefined,
          }),
          replace: true,
        });
        return next;
      });
    },
    [navigate],
  );

  const setEntryRef = useCallback(
    (id: string) => (node: HTMLDivElement | null) => {
      if (node) {
        entryRefs.current.set(id, node);
      } else {
        entryRefs.current.delete(id);
      }
    },
    [],
  );

  // Browse query (used when not searching)
  const browseQuery = useEntries({
    sort: sort ?? "recent",
    kind,
    scope_path,
    tag,
    created_by,
    include_superseded: show_forgotten,
    limit: 20,
  });

  const recallQuery = useAgentRecall(
    {
      query: debouncedQuery || undefined,
      scope: recallScope,
      kinds: recallKinds,
      tags: recallTags,
      limit: recallLimit,
      max_tokens: recallMaxTokens,
    },
    {
      enabled: isRecallMode,
    },
  );

  const agentBrowseQuery = useAgentBrowse(
    {
      scope_path: browseScope,
      kind: browseKind,
      tag: browseTag,
      created_by: browseAgent,
      include_superseded: browseForgotten || undefined,
      limit: 20,
      cursor: browseCursor,
    },
    {
      enabled: isBrowseMode,
    },
  );

  const handleBrowseNext = useCallback(() => {
    const cursor = agentBrowseQuery.data?.next_cursor;
    if (cursor) setBrowseCursor(cursor);
  }, [agentBrowseQuery.data?.next_cursor]);

  const handleBrowseReset = useCallback(() => {
    setBrowseCursor(undefined);
  }, []);

  const browseData = browseQuery.data;
  const browseEntries = useMemo(
    () => browseData?.pages.flatMap((page) => page.items) ?? [],
    [browseData],
  );
  const recallResults = recallQuery.data?.results ?? [];

  // Curate mode uses browseEntries; recall mode uses recallResults rendered separately
  const entries = isRecallMode ? [] : browseEntries;
  const totalCount = isRecallMode
    ? (recallQuery.data?.returned ?? 0)
    : (browseData?.pages[0]?.total ?? 0);
  const isLoading = isRecallMode ? recallQuery.isLoading : browseQuery.isLoading;
  const isError = isRecallMode ? recallQuery.isError : browseQuery.isError;
  const error = isRecallMode ? recallQuery.error : browseQuery.error;
  const hasNextPage = browseQuery.hasNextPage;
  const fetchNextPage = browseQuery.fetchNextPage;
  const isFetchingNextPage = browseQuery.isFetchingNextPage;

  const handleLoadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const sentinelRef = useIntersectionObserver(handleLoadMore, !!hasNextPage && !isFetchingNextPage);

  useLayoutEffect(() => {
    if (!entry_id || isLoading || !expandedIds.has(entry_id)) return;

    let frameOne = 0;
    let frameTwo = 0;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;

    const scrollToSelectedEntry = () => {
      const target = entryRefs.current.get(entry_id);
      if (!target) return;

      const header =
        document.querySelector("header") instanceof HTMLElement
          ? document.querySelector("header")
          : null;
      const headerHeight = header?.getBoundingClientRect().height ?? 0;
      const topGap = 12;
      const top = window.scrollY + target.getBoundingClientRect().top - headerHeight - topGap;

      window.scrollTo({
        top: Math.max(top, 0),
        behavior: "smooth",
      });
    };

    frameOne = window.requestAnimationFrame(() => {
      frameTwo = window.requestAnimationFrame(scrollToSelectedEntry);
    });

    timeoutId = setTimeout(scrollToSelectedEntry, 180);

    return () => {
      window.cancelAnimationFrame(frameOne);
      window.cancelAnimationFrame(frameTwo);
      if (timeoutId) clearTimeout(timeoutId);
    };
  }, [entry_id, expandedIds, isLoading]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-medium tracking-tight">Feed</h2>
          <FeedModeSelect value={activeMode} onChange={handleModeChange} />
          {activeMode === "curate" && (
            <SortSelect value={sort ?? "recent"} onChange={handleSortChange} />
          )}
        </div>
        <div className="flex items-center gap-3">
          {(isRecallMode
            ? recallResults.length > 0
            : isBrowseMode
              ? (agentBrowseQuery.data?.entries.length ?? 0) > 0
              : entries.length > 0) && (
            <span className="font-mono text-xs text-muted-foreground">
              {isRecallMode
                ? recallResults.length
                : isBrowseMode
                  ? (agentBrowseQuery.data?.entries.length ?? 0)
                  : entries.length}
              {isBrowseMode && agentBrowseQuery.data
                ? ` / ${agentBrowseQuery.data.total}`
                : !isRecallMode && totalCount > entries.length
                  ? ` / ${totalCount}`
                  : ""}
              {isRecallMode ? " results" : " entries"}
            </span>
          )}
          <button
            type="button"
            onClick={toggleMergeMode}
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
            onClick={() => setShowNewEntry(true)}
            disabled={showNewEntry || mergeMode || isBrowseMode}
            className="flex items-center gap-1 rounded-md border border-border bg-muted px-2 py-1 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
          >
            <Plus className="h-3 w-3" />
            new
          </button>
        </div>
      </div>

      {!isBrowseMode && (
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            ref={inputRef}
            type="text"
            placeholder={
              isRecallMode
                ? "Recall query (matches cx_recall)..."
                : `Switch to Recall mode to search like MCP...`
            }
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            disabled={!isRecallMode}
            className="pl-8 pr-8 font-mono text-xs"
          />
          {isRecallMode && searchInput && (
            <button
              type="button"
              onClick={handleClearSearch}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      )}

      {activeMode === "curate" && (
        <FilterBar
          filters={{ scope_path, kind, tag, created_by, show_forgotten }}
          onChange={handleFilterChange}
        />
      )}

      {isRecallMode && (
        <RecallBar
          scope={recallScope}
          kinds={recallKinds}
          tags={recallTags}
          limit={recallLimit}
          maxTokens={recallMaxTokens}
          onScopeChange={setRecallScope}
          onKindsChange={setRecallKinds}
          onTagsChange={setRecallTags}
          onLimitChange={(value) => setRecallLimit(Math.max(1, Math.min(200, value)))}
          onMaxTokensChange={setRecallMaxTokens}
          onClear={() => {
            setRecallScope(undefined);
            setRecallKinds([]);
            setRecallTags([]);
            setRecallLimit(20);
            setRecallMaxTokens(undefined);
            setSearchInput("");
          }}
        />
      )}

      {isRecallMode && recallQuery.isLoading && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">Recalling...</p>
        </div>
      )}

      {isRecallMode && recallQuery.isError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            Recall failed: {recallQuery.error?.message ?? "Unknown error"}
          </p>
        </div>
      )}

      {isRecallMode && recallQuery.data && (
        <>
          <TracePanel
            data={{
              kind: "recall",
              trace: recallQuery.data._trace,
              scope_chain: recallQuery.data.scope_chain,
              token_estimate: recallQuery.data.token_estimate,
              returned: recallQuery.data.returned,
            }}
          />
          {recallResults.length === 0 ? (
            <div className="rounded-lg border border-border bg-card p-8 text-center">
              <p className="text-sm text-muted-foreground">
                {debouncedQuery ? `No results for "${debouncedQuery}".` : "No recall results."}
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {recallResults.map((hit) => (
                <SnippetCard
                  key={hit.id}
                  entry={hit}
                  isExpanded={expandedIds.has(hit.id)}
                  onToggle={() => toggleExpanded(hit.id)}
                />
              ))}
            </div>
          )}
        </>
      )}

      {isBrowseMode && (
        <FilterBar
          filters={{
            scope_path: browseScope,
            kind: browseKind,
            tag: browseTag,
            created_by: browseAgent,
            show_forgotten: browseForgotten,
          }}
          onChange={handleBrowseFilterChange}
        />
      )}

      {isBrowseMode && agentBrowseQuery.isLoading && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">Browsing...</p>
        </div>
      )}

      {isBrowseMode && agentBrowseQuery.isError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            Browse failed: {agentBrowseQuery.error?.message ?? "Unknown error"}
          </p>
        </div>
      )}

      {isBrowseMode && agentBrowseQuery.data && (
        <>
          <TracePanel
            data={{
              kind: "browse",
              trace: agentBrowseQuery.data._trace,
              total: agentBrowseQuery.data.total,
              has_more: agentBrowseQuery.data.has_more,
            }}
          />
          {agentBrowseQuery.data.entries.length === 0 ? (
            <div className="rounded-lg border border-border bg-card p-8 text-center">
              <p className="text-sm text-muted-foreground">No entries match the current filters.</p>
            </div>
          ) : (
            <div className="space-y-2">
              {agentBrowseQuery.data.entries.map((entry) => (
                <SnippetCard
                  key={entry.id}
                  entry={entry}
                  isExpanded={expandedIds.has(entry.id)}
                  onToggle={() => toggleExpanded(entry.id)}
                />
              ))}
            </div>
          )}

          <div className="flex items-center justify-between">
            {browseCursor ? (
              <button
                type="button"
                onClick={handleBrowseReset}
                className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                first page
              </button>
            ) : (
              <div />
            )}
            {agentBrowseQuery.data.has_more && agentBrowseQuery.data.next_cursor && (
              <button
                type="button"
                onClick={handleBrowseNext}
                className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                next page
              </button>
            )}
          </div>
        </>
      )}

      {activeMode === "curate" && isLoading && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">Loading entries...</p>
        </div>
      )}

      {activeMode === "curate" && isError && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            Failed to load entries: {error?.message ?? "Unknown error"}
          </p>
        </div>
      )}

      {mergeMode && (
        <div className="space-y-3">
          <div className="flex items-center justify-between rounded-lg border border-border bg-muted/50 px-3 py-2">
            <span className="font-mono text-xs text-muted-foreground">
              {selectedIds.size === 0 ? "Select entries to merge" : `${selectedIds.size} selected`}
            </span>
            {selectedIds.size >= 2 && (
              <span className="font-mono text-[10px] text-muted-foreground/60">
                merge panel below
              </span>
            )}
          </div>
          {selectedIds.size >= 2 && (
            <MergePanel
              entries={entries.filter((e) => selectedIds.has(e.id))}
              onComplete={handleMergeComplete}
              onCancel={toggleMergeMode}
            />
          )}
        </div>
      )}

      {showNewEntry && (
        <NewEntryEditor
          onCancel={() => setShowNewEntry(false)}
          onCreated={() => setShowNewEntry(false)}
        />
      )}

      {activeMode === "curate" &&
        !isLoading &&
        entries.length === 0 &&
        !isError &&
        !showNewEntry && (
          <div className="rounded-lg border border-border bg-card p-8 text-center">
            <p className="text-sm text-muted-foreground">No entries found.</p>
          </div>
        )}

      {activeMode === "curate" && entries.length > 0 && (
        <div className="space-y-2">
          {entries.map((entry) => (
            <div
              key={entry.id}
              ref={setEntryRef(entry.id)}
              className="flex items-start gap-2 scroll-mt-16"
            >
              {mergeMode && (
                <button
                  type="button"
                  onClick={() => toggleSelection(entry.id)}
                  className={`mt-4 flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors ${
                    selectedIds.has(entry.id)
                      ? "border-ring bg-foreground text-background"
                      : "border-border bg-card hover:border-ring/50"
                  }`}
                  aria-label={`Select ${entry.title} for merge`}
                >
                  {selectedIds.has(entry.id) && (
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
                  entry={entry}
                  isExpanded={!mergeMode && expandedIds.has(entry.id)}
                  className={
                    expandedIds.has(entry.id)
                      ? highlightedId === entry.id
                        ? "border-amber-400/40 bg-amber-500/8 ring-2 ring-amber-300/30 transition-all duration-500"
                        : "border-border/90 bg-accent/10 ring-1 ring-ring/25 transition-all duration-300"
                      : undefined
                  }
                  onToggle={
                    mergeMode ? () => toggleSelection(entry.id) : () => toggleExpanded(entry.id)
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

const FEED_MODES: ReadonlySet<string> = new Set(["curate", "recall", "browse"]);

function isFeedMode(v: unknown): v is FeedMode {
  return typeof v === "string" && FEED_MODES.has(v);
}

function isEntryKind(v: unknown): v is EntryKind {
  return typeof v === "string" && ENTRY_KINDS.has(v);
}

function isBrowseSort(v: unknown): v is BrowseSort {
  return typeof v === "string" && BROWSE_SORTS.has(v);
}
