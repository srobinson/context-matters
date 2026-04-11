import { ArrowUpRight } from "lucide-react";
import type React from "react";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { WebBrowseRow } from "@/api/generated/WebBrowseRow";
import type { WebRecallRow } from "@/api/generated/WebRecallRow";
import { KindBadge } from "@/components/domain/KindBadge";
import { cn } from "@/lib/utils";

/**
 * Common row shape for `EntrySummary`. The two generated projection
 * types are structurally similar but recall rows carry a `score`
 * field and always-non-null scope/kind; browse rows hoist those
 * columns to the header. The summary renders only what the row
 * itself carries; hoisted header constants are the caller's job
 * to render above the list via [`HoistedHeader`].
 */
export type EntryRow = WebBrowseRow | WebRecallRow;

interface EntrySummaryProps {
  row: EntryRow;
  className?: string;
  /** When true, parse `«term»` markers in the snippet and wrap them in `<mark>`. */
  highlightMatches?: boolean;
  /** When true, show a trailing arrow indicator for link-style rows. */
  showArrow?: boolean;
}

function isRecallRow(row: EntryRow): row is WebRecallRow {
  return "score" in row;
}

/**
 * Parses `«term»` YAML-safe highlight brackets in a recall snippet and
 * wraps each match in `<mark>`. When `highlight` is false or the
 * snippet contains no brackets, returns the raw string unchanged.
 */
function renderSnippet(snippet: string, highlight: boolean): React.ReactNode {
  if (!highlight || !snippet.includes("«")) return snippet;
  const parts: React.ReactNode[] = [];
  let last = 0;
  const re = /«([^»]*)»/g;
  let match: RegExpExecArray | null = re.exec(snippet);
  let idx = 0;
  while (match !== null) {
    if (match.index > last) parts.push(snippet.slice(last, match.index));
    parts.push(
      <mark
        key={`m${idx++}`}
        className="rounded-sm bg-amber-300/30 px-0.5 text-foreground dark:bg-amber-400/25"
      >
        {match[1]}
      </mark>,
    );
    last = match.index + match[0].length;
    match = re.exec(snippet);
  }
  if (last < snippet.length) parts.push(snippet.slice(last));
  return parts;
}

export function EntrySummary({
  row,
  className,
  highlightMatches = false,
  showArrow = false,
}: EntrySummaryProps) {
  const score = isRecallRow(row) ? row.score : null;

  return (
    <div className={cn("flex items-start gap-3", className)}>
      <div className="min-w-0 flex-1 space-y-1.5">
        <div className="flex items-start justify-between gap-3">
          <p className="line-clamp-2 text-sm font-medium leading-snug text-foreground">
            {row.title}
          </p>
          <div className="flex shrink-0 items-center gap-2">
            {score != null && (
              <span
                className="rounded-md border border-ring/30 bg-ring/10 px-1.5 py-0.5 font-mono text-[10px] leading-none text-foreground"
                title={`score ${score.toFixed(2)}`}
              >
                {score.toFixed(2)}
              </span>
            )}
            <span className="font-mono text-[10px] text-muted-foreground/70" title={row.id}>
              {row.age}
            </span>
          </div>
        </div>

        {row.snippet && (
          <p className="line-clamp-2 font-mono text-xs leading-relaxed text-muted-foreground">
            {renderSnippet(row.snippet, highlightMatches)}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[10px] text-muted-foreground">
          {row.kind && <KindBadge kind={row.kind as EntryKind} className="shrink-0" />}
          {row.scope && <span className="truncate">{row.scope}</span>}
          {row.tags.length > 0 && (
            <span className="truncate">
              {row.tags.slice(0, 3).join(", ")}
              {row.tags.length > 3 && ` +${row.tags.length - 3}`}
            </span>
          )}
        </div>
      </div>
      {showArrow && (
        <ArrowUpRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground/30 transition-colors group-hover:text-muted-foreground" />
      )}
    </div>
  );
}
