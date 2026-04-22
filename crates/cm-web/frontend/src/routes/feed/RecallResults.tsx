import type { RecallView } from "@/api/client";
import type { WebRecallRow } from "@/api/generated/WebRecallRow";
import { SnippetCard } from "@/components/SnippetCard";
import { TracePanel } from "@/components/TracePanel";

interface RecallResultsProps {
  isLoading: boolean;
  isError: boolean;
  errorMessage?: string;
  data?: RecallView;
  entries: WebRecallRow[];
  debouncedQuery: string;
  expandedIds: Set<string>;
  onToggleExpanded: (id: string) => void;
}

export function RecallResults({
  isLoading,
  isError,
  errorMessage,
  data,
  entries,
  debouncedQuery,
  expandedIds,
  onToggleExpanded,
}: RecallResultsProps) {
  if (isLoading) {
    return (
      <div className="rounded-lg border border-border bg-card p-8 text-center">
        <p className="text-sm text-muted-foreground">Recalling...</p>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
        <p className="text-sm text-destructive">Recall failed: {errorMessage ?? "Unknown error"}</p>
      </div>
    );
  }

  if (!data) return null;

  return (
    <>
      <TracePanel
        data={{
          kind: "recall",
          header: data.header,
          advisories: data.advisories,
        }}
      />
      {entries.length === 0 ? (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-sm text-muted-foreground">
            {debouncedQuery ? `No results for "${debouncedQuery}".` : "No recall results."}
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {entries.map((hit) => (
            <SnippetCard
              key={hit.id}
              row={hit}
              highlightMatches
              isExpanded={expandedIds.has(hit.id)}
              onToggle={() => onToggleExpanded(hit.id)}
            />
          ))}
        </div>
      )}
      {data.advisories.length > 0 && (
        <div className="rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2">
          <div className="mb-1 font-mono text-[10px] uppercase tracking-wider text-amber-600/80 dark:text-amber-400/80">
            advisories
          </div>
          <ul className="space-y-0.5 font-mono text-[11px] text-amber-700 dark:text-amber-300">
            {data.advisories.map((msg) => (
              <li key={msg}>{msg}</li>
            ))}
          </ul>
        </div>
      )}
    </>
  );
}
