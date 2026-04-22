import { useCallback, useState } from "react";
import type { EntryKind } from "@/api/generated/EntryKind";

export function useRecallControls() {
  const [recallScope, setRecallScope] = useState<string | undefined>(undefined);
  const [recallKinds, setRecallKinds] = useState<EntryKind[]>([]);
  const [recallTags, setRecallTags] = useState<string[]>([]);
  const [recallLimit, setRecallLimit] = useState(20);
  const [recallMaxTokens, setRecallMaxTokens] = useState<number | undefined>(undefined);

  const resetRecallControls = useCallback(() => {
    setRecallScope(undefined);
    setRecallKinds([]);
    setRecallTags([]);
    setRecallLimit(20);
    setRecallMaxTokens(undefined);
  }, []);

  const setClampedRecallLimit = useCallback((value: number) => {
    setRecallLimit(Math.max(1, Math.min(200, value)));
  }, []);

  return {
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
  };
}
