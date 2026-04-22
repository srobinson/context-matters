import { useQueries } from "@tanstack/react-query";
import { useCallback, useMemo, useState } from "react";
import { api, type EntryDetail } from "@/api/client";
import { queryKeys } from "@/api/hooks";

interface UseMergeSelectionOptions {
  clearExpanded: () => void;
}

export function useMergeSelection({ clearExpanded }: UseMergeSelectionOptions) {
  const [mergeMode, setMergeMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const resetMerge = useCallback(() => {
    setMergeMode(false);
    setSelectedIds(new Set());
  }, []);

  const toggleMergeMode = useCallback(() => {
    setMergeMode((prev) => {
      if (prev) setSelectedIds(new Set());
      return !prev;
    });
    clearExpanded();
  }, [clearExpanded]);

  const toggleSelection = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const selectedIdList = useMemo(() => [...selectedIds], [selectedIds]);
  const selectedEntryQueries = useQueries({
    queries: selectedIdList.map((id) => ({
      queryKey: queryKeys.entries.detail(id),
      queryFn: () => api.entries.get(id),
      enabled: mergeMode && selectedIdList.length >= 2,
    })),
  });
  const hydratedSelectedEntries = selectedEntryQueries
    .map((query) => query.data)
    .filter((detail): detail is EntryDetail => !!detail);
  const selectionHydrated =
    hydratedSelectedEntries.length === selectedIdList.length && selectedIdList.length > 0;

  return {
    mergeMode,
    selectedIds,
    hydratedSelectedEntries,
    selectionHydrated,
    resetMerge,
    toggleMergeMode,
    toggleSelection,
  };
}
