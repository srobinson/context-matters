import {
  useQuery,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  type UseQueryOptions,
} from "@tanstack/react-query";
import {
  api,
  type BrowseParams,
  type EntryDetail,
  type SearchParams,
  type MutationListParams,
  type PagedResponse,
  type Stats,
} from "./client";
import type { Entry } from "./generated/Entry";
import type { MutationRecord } from "./generated/MutationRecord";
import type { NewEntry } from "./generated/NewEntry";
import type { UpdateEntry } from "./generated/UpdateEntry";

// --- Query key factory ---

export const queryKeys = {
  entries: {
    all: ["entries"] as const,
    browse: (params: BrowseParams) => ["entries", "browse", params] as const,
    detail: (id: string) => ["entries", "detail", id] as const,
    search: (params: SearchParams) => ["entries", "search", params] as const,
  },
  stats: ["stats"] as const,
  mutations: {
    all: ["mutations"] as const,
    list: (params: MutationListParams) =>
      ["mutations", "list", params] as const,
  },
};

// --- Query hooks ---

export function useEntries(params: Omit<BrowseParams, "cursor"> = {}) {
  return useInfiniteQuery({
    queryKey: queryKeys.entries.browse(params),
    queryFn: ({ pageParam }) =>
      api.entries.browse({ ...params, cursor: pageParam }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.next_cursor ?? undefined,
  });
}

export function useEntry(
  id: string,
  options?: Partial<UseQueryOptions<EntryDetail>>,
) {
  return useQuery({
    queryKey: queryKeys.entries.detail(id),
    queryFn: () => api.entries.get(id),
    enabled: !!id,
    ...options,
  });
}

export function useStats(options?: Partial<UseQueryOptions<Stats>>) {
  return useQuery({
    queryKey: queryKeys.stats,
    queryFn: () => api.stats.get(),
    ...options,
  });
}

export function useSearch(
  params: SearchParams,
  options?: Partial<UseQueryOptions<PagedResponse<Entry>>>,
) {
  return useQuery({
    queryKey: queryKeys.entries.search(params),
    queryFn: () => api.entries.search(params),
    enabled: !!params.query,
    ...options,
  });
}

export function useMutationHistory(
  params: MutationListParams = {},
  options?: Partial<UseQueryOptions<PagedResponse<MutationRecord>>>,
) {
  return useQuery({
    queryKey: queryKeys.mutations.list(params),
    queryFn: () => api.mutations.list(params),
    ...options,
  });
}

// --- Mutation hooks ---

export function useCreateEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (entry: NewEntry) => api.entries.create(entry),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.entries.all });
      queryClient.invalidateQueries({ queryKey: queryKeys.stats });
    },
  });
}

export function useUpdateEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, update }: { id: string; update: UpdateEntry }) =>
      api.entries.update(id, update),
    onSuccess: (_data, { id }) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.entries.detail(id) });
      queryClient.invalidateQueries({ queryKey: queryKeys.entries.all });
    },
  });
}

export function useForgetEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.entries.forget(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.entries.all });
      queryClient.invalidateQueries({ queryKey: queryKeys.stats });
    },
  });
}
