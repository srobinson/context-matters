import { useCallback, useState } from "react";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { SingularScopeSelector } from "@/components/domain/ScopeSelector";
import { useScopeSelectorState } from "@/hooks/useScopeSelectorState";
import type { ScopeSelector } from "@/lib/scope";

export function useRecallControls() {
  const [recallScopeValue, setRecallScopeValue] = useScopeSelectorState();
  const [recallKinds, setRecallKinds] = useState<EntryKind[]>([]);
  const [recallTags, setRecallTags] = useState<string[]>([]);
  const [recallLimit, setRecallLimit] = useState(20);
  const [recallMaxTokens, setRecallMaxTokens] = useState<number | undefined>(undefined);

  const resetRecallControls = useCallback(() => {
    setRecallScopeValue(undefined);
    setRecallKinds([]);
    setRecallTags([]);
    setRecallLimit(20);
    setRecallMaxTokens(undefined);
  }, [setRecallScopeValue]);

  const setClampedRecallLimit = useCallback((value: number) => {
    setRecallLimit(Math.max(1, Math.min(200, value)));
  }, []);

  return {
    recallScope: toSingularScope(recallScopeValue),
    recallKinds,
    recallTags,
    recallLimit,
    recallMaxTokens,
    resetRecallControls,
    setRecallScope: setRecallScopeValue,
    setRecallKinds,
    setRecallTags,
    setClampedRecallLimit,
    setRecallMaxTokens,
  };
}

function toSingularScope(scope: ScopeSelector | undefined): SingularScopeSelector | undefined {
  if (scope?.kind === "path" || scope?.kind === "cwd_inferred") {
    return scope;
  }
  return undefined;
}
