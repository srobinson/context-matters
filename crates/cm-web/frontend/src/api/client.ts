import type { Entry } from "./generated/Entry";
import type { EntryKind } from "./generated/EntryKind";
import type { EntryRelation } from "./generated/EntryRelation";
import type { BrowseSort } from "./generated/BrowseSort";
import type { MutationRecord } from "./generated/MutationRecord";
import type { NewEntry } from "./generated/NewEntry";
import type { UpdateEntry } from "./generated/UpdateEntry";
import type { StoreStats } from "./generated/StoreStats";

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
  params: Record<string, string | number | boolean | undefined | null>,
): string {
  const sp = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
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

// --- Entry detail (GET /entries/:id response) ---

export type EntryDetail = Entry & {
  relations_from: EntryRelation[];
  relations_to: EntryRelation[];
};

// --- Param types ---

export interface BrowseParams {
  scope_path?: string;
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
  scope_path?: string;
  kind?: EntryKind;
  tag?: string;
  limit?: number;
  cursor?: string;
}

export interface MutationListParams {
  entry_id?: string;
  limit?: number;
  cursor?: string;
}

// --- API namespace ---

export const api = {
  entries: {
    browse(params: BrowseParams = {}): Promise<PagedResponse<Entry>> {
      return apiFetch(
        `/entries${toSearchParams({
          scope_path: params.scope_path,
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

    search(params: SearchParams): Promise<PagedResponse<Entry>> {
      return apiFetch(
        `/entries/search${toSearchParams({
          query: params.query,
          scope_path: params.scope_path,
          kind: params.kind,
          tag: params.tag,
          limit: params.limit,
          cursor: params.cursor,
        })}`,
      );
    },

    get(id: string): Promise<EntryDetail> {
      return apiFetch(`/entries/${encodeURIComponent(id)}`);
    },

    create(entry: NewEntry): Promise<Entry> {
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

    merge(oldId: string, newEntry: NewEntry): Promise<Entry> {
      return apiFetch("/entries/merge", {
        method: "POST",
        body: JSON.stringify({ old_id: oldId, new_entry: newEntry }),
      });
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
          limit: params.limit,
          cursor: params.cursor,
        })}`,
      );
    },
  },

  export(): Promise<Blob> {
    return fetch(`${API_BASE}/export`).then((res) => {
      if (!res.ok) throw new ApiError(res.status, null);
      return res.blob();
    });
  },
};
