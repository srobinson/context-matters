import type { AgentSearchParams, RecallParams } from "@/api/client";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useAgentRecall, useAgentSearch } from "@/api/hooks";
import type { SingularScopeSelector } from "@/components/domain/ScopeSelector";
import type { ScopeSelector } from "@/lib/scope";

const ALL_SCOPE: ScopeSelector = { kind: "all" };

interface UseRecallOrSearchArgs {
  isRecallMode: boolean;
  scope?: SingularScopeSelector;
  debouncedQuery: string;
  kinds: EntryKind[];
  tags: string[];
  limit: number;
  maxTokens?: number;
}

export function useRecallOrSearch({
  isRecallMode,
  scope,
  debouncedQuery,
  kinds,
  tags,
  limit,
  maxTokens,
}: UseRecallOrSearchArgs) {
  const selector = scope ?? ALL_SCOPE;
  const isAllScope = selector.kind === "all";
  const recallParams: RecallParams = {
    query: debouncedQuery || undefined,
    scope,
    kinds,
    tags,
    limit,
    max_tokens: maxTokens,
  };
  const searchParams: AgentSearchParams = {
    query: debouncedQuery,
    scope: ALL_SCOPE,
    kind: first(kinds),
    tag: first(tags),
    limit,
  };

  const recallQuery = useAgentRecall(recallParams, {
    enabled: isRecallMode && !isAllScope,
  });
  const searchQuery = useAgentSearch(searchParams, {
    enabled: isRecallMode && isAllScope && debouncedQuery.length > 0,
  });

  return {
    query: isAllScope ? searchQuery : recallQuery,
    showQueryOrScopeHint: isRecallMode && isAllScope && debouncedQuery.length === 0,
  };
}

function first<T>(values: T[]): T | undefined {
  return values[0];
}
