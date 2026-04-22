import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

interface UseEntryFocusOptions {
  entryId?: string;
  isLoading: boolean;
  onEntryIdChange: (entryId?: string) => void;
}

export function useEntryFocus({ entryId, isLoading, onEntryIdChange }: UseEntryFocusOptions) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [highlightedId, setHighlightedId] = useState<string | null>(null);
  const entryRefs = useRef(new Map<string, HTMLDivElement>());

  useEffect(() => {
    if (entryId) {
      setExpandedIds((prev) => {
        if (prev.has(entryId)) return prev;
        const next = new Set(prev);
        next.add(entryId);
        return next;
      });
    }
  }, [entryId]);

  useEffect(() => {
    if (!entryId) {
      setHighlightedId(null);
      return;
    }

    setHighlightedId(entryId);
    const timeoutId = window.setTimeout(() => {
      setHighlightedId((current) => (current === entryId ? null : current));
    }, 1800);

    return () => window.clearTimeout(timeoutId);
  }, [entryId]);

  const clearExpanded = useCallback(() => {
    setExpandedIds(new Set());
  }, []);

  const toggleExpanded = useCallback(
    (id: string) => {
      setExpandedIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        onEntryIdChange(next.size === 1 ? [...next][0] : undefined);
        return next;
      });
    },
    [onEntryIdChange],
  );

  const setEntryRef = useCallback(
    (id: string) => (node: HTMLDivElement | null) => {
      if (node) {
        entryRefs.current.set(id, node);
      } else {
        entryRefs.current.delete(id);
      }
    },
    [],
  );

  useLayoutEffect(() => {
    if (!entryId || isLoading || !expandedIds.has(entryId)) return;

    let frameOne = 0;
    let frameTwo = 0;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;

    const scrollToSelectedEntry = () => {
      const target = entryRefs.current.get(entryId);
      if (!target) return;

      const header =
        document.querySelector("header") instanceof HTMLElement
          ? document.querySelector("header")
          : null;
      const headerHeight = header?.getBoundingClientRect().height ?? 0;
      const topGap = 12;
      const top = window.scrollY + target.getBoundingClientRect().top - headerHeight - topGap;

      window.scrollTo({
        top: Math.max(top, 0),
        behavior: "smooth",
      });
    };

    frameOne = window.requestAnimationFrame(() => {
      frameTwo = window.requestAnimationFrame(scrollToSelectedEntry);
    });

    timeoutId = setTimeout(scrollToSelectedEntry, 180);

    return () => {
      window.cancelAnimationFrame(frameOne);
      window.cancelAnimationFrame(frameTwo);
      if (timeoutId) clearTimeout(timeoutId);
    };
  }, [entryId, expandedIds, isLoading]);

  return {
    expandedIds,
    highlightedId,
    clearExpanded,
    setEntryRef,
    toggleExpanded,
  };
}
