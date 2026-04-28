import type { BrowseSort } from "@/api/generated/BrowseSort";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { FeedMode } from "@/components/domain/FeedModeSelect";

export type FeedSearch = {
  mode?: FeedMode;
  scope?: string;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  sort?: BrowseSort;
  show_forgotten?: boolean;
  q?: string;
  entry_id?: string;
};

export function validateFeedSearch(search: Record<string, unknown>): FeedSearch {
  const scope =
    typeof search.scope === "string"
      ? search.scope
      : typeof search.scope_path === "string"
        ? search.scope_path
        : undefined;

  return {
    mode: isFeedMode(search.mode) ? search.mode : undefined,
    scope,
    kind: isEntryKind(search.kind) ? search.kind : undefined,
    tag: typeof search.tag === "string" ? search.tag : undefined,
    created_by: typeof search.created_by === "string" ? search.created_by : undefined,
    sort: isBrowseSort(search.sort) ? search.sort : undefined,
    show_forgotten: search.show_forgotten === true || search.show_forgotten === "true",
    q: typeof search.q === "string" && search.q ? search.q : undefined,
    entry_id: typeof search.entry_id === "string" && search.entry_id ? search.entry_id : undefined,
  };
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
