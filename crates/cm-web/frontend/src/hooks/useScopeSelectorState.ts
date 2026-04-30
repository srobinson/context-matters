import { useCallback, useEffect, useState } from "react";
import type { ScopeSelector } from "@/lib/scope";

export function useScopeSelectorState(
  initialValue?: ScopeSelector,
  onValueChange?: (value: ScopeSelector | undefined) => void,
) {
  const [value, setInternalValue] = useState<ScopeSelector | undefined>(initialValue);

  useEffect(() => {
    setInternalValue(initialValue);
  }, [initialValue]);

  const setValue = useCallback(
    (nextValue: ScopeSelector | undefined) => {
      setInternalValue(nextValue);
      onValueChange?.(nextValue);
    },
    [onValueChange],
  );

  return [value, setValue] as const;
}
