import type { ScopeSelector } from "@/lib/scope";
import { serializeScopeSelector } from "@/lib/scope";
import type { BrowseSort } from "./generated/BrowseSort";
import type { Entry } from "./generated/Entry";
import type { EntryKind } from "./generated/EntryKind";
import type { EntryMeta } from "./generated/EntryMeta";
import type { EntryRelation } from "./generated/EntryRelation";
import type { MutationRecord } from "./generated/MutationRecord";
import type { StoreStats } from "./generated/StoreStats";
import type { UpdateEntry } from "./generated/UpdateEntry";
import type { WebBrowseView } from "./generated/WebBrowseView";
import type { WebRecallView } from "./generated/WebRecallView";

const API_BASE = "/api";

// --- Error type ---

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: unknown,
  ) {
    super(`API ${status}`);
    this.name = "ApiError";
  }
}

// --- Base fetch ---

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });

  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new ApiError(res.status, body);
  }

  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

// --- Query param serialization ---

function toSearchParams(
  params: Record<
    string,
    string | number | boolean | undefined | null | Array<string | number | boolean>
  >,
): string {
  const sp = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (Array.isArray(value)) {
      for (const item of value) {
        if (item != null && item !== "") {
          sp.append(key, String(item));
        }
      }
      continue;
    }
    if (value != null && value !== "") {
      sp.set(key, String(value));
    }
  }
  const str = sp.toString();
  return str ? `?${str}` : "";
}

// --- Response types ---
// ts-rs maps Rust u64/i64 to bigint, but JSON.parse produces number.
// These aliases correct the mismatch for API responses.

export type PagedResponse<T> = {
  items: T[];
  total: number;
  next_cursor?: string | null;
};

export type Stats = Omit<
  StoreStats,
  | "active_entries"
  | "superseded_entries"
  | "scopes"
  | "relations"
  | "db_size_bytes"
  | "entries_by_kind"
  | "entries_by_scope"
  | "entries_by_tag"
> & {
  active_entries: number;
  superseded_entries: number;
  scopes: number;
  relations: number;
  entries_by_kind: Record<string, number>;
  entries_by_scope: Record<string, number>;
  entries_by_tag: { tag: string; count: number }[];
  db_size_bytes: number;
  entries_today: number;
  entries_this_week: number;
  active_agents: { created_by: string; count: number }[];
  scope_tree: { path: string; kind: string; entry_count: number }[];
  quality: {
    untagged_count: number;
    stale_count: number;
    global_scope_count: number;
  };
};

// ts-rs maps Rust u64 to bigint, but wire JSON carries a plain number.
// Override the one bigint field on WebBrowseHeader so the rest of the
// app can rely on a pure-number contract without coercion at every
// call site. Recall header is already all numbers.
export type BrowseView = Omit<WebBrowseView, "header"> & {
  header: Omit<WebBrowseView["header"], "total"> & { total: number };
};
export type RecallView = WebRecallView;

// --- Entry detail (GET /entries/:id response) ---

export type EntryDetail = Entry & {
  relations_from: EntryRelation[];
  relations_to: EntryRelation[];
};

// --- Param types ---

export interface BrowseParams {
  scope?: ScopeSelector;
  include_resolution?: boolean;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  include_superseded?: boolean;
  sort?: BrowseSort;
  limit?: number;
  cursor?: string;
}

export interface SearchParams {
  query: string;
  scope?: ScopeSelector;
  kind?: EntryKind[];
  tag?: string[];
  limit?: number;
}

export type AgentSearchParams = SearchParams;

export interface MutationListParams {
  entry_id?: string;
  source?: string;
  action?: string;
  limit?: number;
  cursor?: string;
}

export interface RecallParams {
  query?: string;
  scope?: ScopeSelector;
  kinds?: EntryKind[];
  tags?: string[];
  limit?: number;
  max_tokens?: number;
}

export interface ExportParams {
  scope?: ScopeSelector;
}

export interface AgentBrowseParams {
  scope?: ScopeSelector;
  include_resolution?: boolean;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  include_superseded?: boolean;
  sort?: BrowseSort;
  limit?: number;
  cursor?: string;
}

export interface NewEntryRequest {
  scope: string;
  kind: EntryKind;
  title: string;
  body: string;
  created_by: string;
  meta?: EntryMeta | null;
}

// --- API namespace ---

export const api = {
  entries: {
    browse(params: BrowseParams = {}): Promise<BrowseView> {
      return apiFetch(
        `/entries${toSearchParams({
          scope: serializeScopeSelector(params.scope),
          include_resolution: params.include_resolution,
          kind: params.kind,
          tag: params.tag,
          created_by: params.created_by,
          include_superseded: params.include_superseded,
          sort: params.sort,
          limit: params.limit,
          cursor: params.cursor,
        })}`,
      );
    },

    search(params: SearchParams): Promise<RecallView> {
      return apiFetch(
        `/entries/search${toSearchParams({
          query: params.query,
          scope: serializeScopeSelector(params.scope),
          kind: params.kind,
          tag: params.tag,
          limit: params.limit,
        })}`,
      );
    },

    recall(params: RecallParams): Promise<RecallView> {
      return apiFetch(
        `/entries/recall${toSearchParams({
          query: params.query,
          scope: serializeScopeSelector(params.scope),
          kinds: params.kinds,
          tags: params.tags,
          limit: params.limit,
          max_tokens: params.max_tokens,
        })}`,
      );
    },

    get(id: string): Promise<EntryDetail> {
      return apiFetch(`/entries/${encodeURIComponent(id)}`);
    },

    create(entry: NewEntryRequest): Promise<Entry> {
      return apiFetch("/entries", {
        method: "POST",
        body: JSON.stringify(entry),
      });
    },

    update(id: string, update: UpdateEntry): Promise<Entry> {
      return apiFetch(`/entries/${encodeURIComponent(id)}`, {
        method: "PATCH",
        body: JSON.stringify(update),
      });
    },

    forget(id: string): Promise<void> {
      return apiFetch(`/entries/${encodeURIComponent(id)}`, {
        method: "DELETE",
      });
    },

    merge(oldId: string, newEntry: NewEntryRequest): Promise<Entry> {
      return apiFetch("/entries/merge", {
        method: "POST",
        body: JSON.stringify({ old_id: oldId, new_entry: newEntry }),
      });
    },
  },

  agent: {
    recall(params: RecallParams): Promise<RecallView> {
      return apiFetch(
        `/agent/recall${toSearchParams({
          query: params.query,
          scope: serializeScopeSelector(params.scope),
          kinds: params.kinds,
          tags: params.tags,
          limit: params.limit,
          max_tokens: params.max_tokens,
        })}`,
      );
    },

    search(params: AgentSearchParams): Promise<RecallView> {
      return apiFetch(
        `/agent/search${toSearchParams({
          query: params.query,
          scope: serializeScopeSelector(params.scope),
          kind: params.kind,
          tag: params.tag,
          limit: params.limit,
        })}`,
      );
    },

    browse(params: AgentBrowseParams = {}): Promise<BrowseView> {
      return apiFetch(
        `/agent/browse${toSearchParams({
          scope: serializeScopeSelector(params.scope),
          include_resolution: params.include_resolution,
          kind: params.kind,
          tag: params.tag,
          created_by: params.created_by,
          include_superseded: params.include_superseded,
          sort: params.sort,
          limit: params.limit,
          cursor: params.cursor,
        })}`,
      );
    },
  },

  stats: {
    get(): Promise<Stats> {
      return apiFetch("/stats");
    },
  },

  mutations: {
    list(params: MutationListParams = {}): Promise<PagedResponse<MutationRecord>> {
      return apiFetch(
        `/mutations${toSearchParams({
          entry_id: params.entry_id,
          source: params.source,
          action: params.action,
          limit: params.limit,
          cursor: params.cursor,
        })}`,
      );
    },
  },

  export(params?: ExportParams): Promise<Blob> {
    const query = toSearchParams({ scope: serializeScopeSelector(params?.scope) });
    return fetch(`${API_BASE}/export${query}`).then((res) => {
      if (!res.ok) throw new ApiError(res.status, null);
      return res.blob();
    });
  },
};
