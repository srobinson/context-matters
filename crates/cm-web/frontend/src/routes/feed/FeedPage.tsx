import { useNavigate } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { BrowseSort } from "@/api/generated/BrowseSort";
import type { WebBrowseRow } from "@/api/generated/WebBrowseRow";
import { useEntries } from "@/api/hooks";
import { BrowsePane } from "@/components/BrowsePane";
import { FilterBar, type FilterState } from "@/components/FilterBar";
import { NewEntryEditor } from "@/components/NewEntryEditor";
import { RecallBar } from "@/components/RecallBar";
import { useDebounce } from "@/hooks/useDebounce";
import { useIntersectionObserver } from "@/hooks/useIntersectionObserver";
import { CurateEntries } from "./CurateEntries";
import { FeedSearchInput } from "./FeedSearchInput";
import { FeedToolbar } from "./FeedToolbar";
import { MergeStatus } from "./MergeStatus";
import { RecallResults } from "./RecallResults";
import { type FeedSearch, feedScopeFromScopeSelector, scopeSelectorFromFeedScope } from "./search";
import { useEntryFocus } from "./useEntryFocus";
import { useMergeSelection } from "./useMergeSelection";
import { useRecallControls } from "./useRecallControls";
import { useRecallOrSearch } from "./useRecallOrSearch";

interface FeedPageProps {
  search: FeedSearch;
}

export function FeedPage({ search }: FeedPageProps) {
  const { mode, sort, kind, scope, tag, created_by, show_forgotten, q, entry_id } = search;

  const navigate = useNavigate({ from: "/feed" });
  const [showNewEntry, setShowNewEntry] = useState(false);
  const [searchInput, setSearchInput] = useState(q ?? "");
  const inputRef = useRef<HTMLInputElement>(null);
  const debouncedQuery = useDebounce(searchInput, 300);
  const {
    recallScope,
    recallKinds,
    recallTags,
    recallLimit,
    recallMaxTokens,
    resetRecallControls,
    setRecallScope,
    setRecallKinds,
    setRecallTags,
    setClampedRecallLimit,
    setRecallMaxTokens,
  } = useRecallControls();

  const activeMode = mode ?? "curate";
  const isRecallMode = activeMode === "recall";
  const isBrowseMode = activeMode === "browse";

  useEffect(() => {
    navigate({
      search: (prev) => ({
        ...prev,
        scope,
      }),
      replace: true,
    });
  }, [scope, navigate]);

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

  const handleModeChange = useCallback(
    (nextMode: typeof activeMode) => {
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
      const { scope: nextScope, ...rest } = update;
      const nextSearch: Partial<FeedSearch> = { ...rest };
      if ("scope" in update) {
        nextSearch.scope = feedScopeFromScopeSelector(nextScope);
      }
      navigate({
        search: (prev) => ({ ...prev, ...nextSearch }),
      });
    },
    [navigate],
  );

  const handleClearSearch = useCallback(() => {
    setSearchInput("");
    inputRef.current?.focus();
  }, []);

  const browseQuery = useEntries({
    sort: sort ?? "recent",
    kind,
    scope: scopeSelectorFromFeedScope(scope),
    tag,
    created_by,
    include_superseded: show_forgotten,
    limit: 20,
  });

  const { query: recallQuery, showQueryOrScopeHint } = useRecallOrSearch({
    isRecallMode,
    scope: recallScope,
    query: searchInput,
    debouncedQuery,
    kinds: recallKinds,
    tags: recallTags,
    limit: recallLimit,
    maxTokens: recallMaxTokens,
  });

  const browseData = browseQuery.data;
  const browseEntries = useMemo<WebBrowseRow[]>(
    () => browseData?.pages.flatMap((page) => page.entries) ?? [],
    [browseData],
  );
  const browseHeader = browseData?.pages[0]?.header;
  const recallEntries = recallQuery.data?.entries ?? [];
  const entries = isRecallMode ? [] : browseEntries;
  const totalCount = isRecallMode
    ? (recallQuery.data?.header.returned ?? 0)
    : (browseData?.pages[0]?.header.total ?? 0);
  const isLoading = isRecallMode ? recallQuery.isLoading : browseQuery.isLoading;
  const isError = isRecallMode ? recallQuery.isError : browseQuery.isError;
  const error = isRecallMode ? recallQuery.error : browseQuery.error;
  const hasNextPage = browseQuery.hasNextPage;
  const fetchNextPage = browseQuery.fetchNextPage;
  const isFetchingNextPage = browseQuery.isFetchingNextPage;

  const updateEntryId = useCallback(
    (nextEntryId?: string) => {
      navigate({
        search: (prevSearch) => ({
          ...prevSearch,
          entry_id: nextEntryId,
        }),
        replace: true,
      });
    },
    [navigate],
  );

  const { expandedIds, highlightedId, clearExpanded, setEntryRef, toggleExpanded } = useEntryFocus({
    entryId: entry_id,
    isLoading,
    onEntryIdChange: updateEntryId,
  });

  const {
    mergeMode,
    selectedIds,
    hydratedSelectedEntries,
    selectionHydrated,
    resetMerge,
    toggleMergeMode,
    toggleSelection,
  } = useMergeSelection({ clearExpanded });

  useEffect(() => {
    if (!isRecallMode) {
      setSearchInput("");
      resetRecallControls();
    }
    if (activeMode !== "curate" && mergeMode) {
      resetMerge();
    }
  }, [activeMode, isRecallMode, mergeMode, resetMerge, resetRecallControls]);

  const handleLoadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const sentinelRef = useIntersectionObserver(handleLoadMore, !!hasNextPage && !isFetchingNextPage);

  return (
    <div className="space-y-4">
      <FeedToolbar
        activeMode={activeMode}
        sort={sort ?? "recent"}
        entriesCount={entries.length}
        totalCount={totalCount}
        mergeMode={mergeMode}
        showNewEntry={showNewEntry}
        isBrowseMode={isBrowseMode}
        onModeChange={handleModeChange}
        onSortChange={handleSortChange}
        onToggleMergeMode={toggleMergeMode}
        onShowNewEntry={() => setShowNewEntry(true)}
      />

      {!isBrowseMode && (
        <FeedSearchInput
          inputRef={inputRef}
          isRecallMode={isRecallMode}
          searchInput={searchInput}
          onSearchInputChange={setSearchInput}
          onClearSearch={handleClearSearch}
        />
      )}

      {activeMode === "curate" && (
        <FilterBar
          filters={{
            scope: scopeSelectorFromFeedScope(scope),
            kind,
            tag,
            created_by,
            show_forgotten,
          }}
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
          onLimitChange={setClampedRecallLimit}
          onMaxTokensChange={setRecallMaxTokens}
          onClear={() => {
            resetRecallControls();
            setSearchInput("");
          }}
        />
      )}

      {isRecallMode && showQueryOrScopeHint && (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">Enter a query or pick a scope.</p>
        </div>
      )}

      {isRecallMode && !showQueryOrScopeHint && (
        <RecallResults
          isLoading={recallQuery.isLoading}
          isError={recallQuery.isError}
          errorMessage={recallQuery.error?.message}
          data={recallQuery.data}
          entries={recallEntries}
          debouncedQuery={debouncedQuery}
          expandedIds={expandedIds}
          onToggleExpanded={toggleExpanded}
        />
      )}

      {isBrowseMode && <BrowsePane expandedIds={expandedIds} onToggleExpanded={toggleExpanded} />}

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
        <MergeStatus
          selectedCount={selectedIds.size}
          selectionHydrated={selectionHydrated}
          hydratedSelectedEntries={hydratedSelectedEntries}
          onComplete={resetMerge}
          onCancel={toggleMergeMode}
        />
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
        <CurateEntries
          entries={entries}
          browseHeader={browseHeader}
          expandedIds={expandedIds}
          highlightedId={highlightedId}
          mergeMode={mergeMode}
          selectedIds={selectedIds}
          isFetchingNextPage={isFetchingNextPage}
          sentinelRef={sentinelRef}
          setEntryRef={setEntryRef}
          onToggleExpanded={toggleExpanded}
          onToggleSelection={toggleSelection}
        />
      )}
    </div>
  );
}
