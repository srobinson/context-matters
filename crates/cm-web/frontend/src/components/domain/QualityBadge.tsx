import type { Entry } from "@/api/generated/Entry";
import { isStale } from "@/lib/time";
import { cn } from "@/lib/utils";

export type QualityIssue = "untagged" | "under-tagged" | "stale";

export function getQualityIssues(entry: Entry): QualityIssue[] {
  const issues: QualityIssue[] = [];
  const tags = entry.meta?.tags ?? [];

  if (tags.length === 0) issues.push("untagged");
  else if (tags.length === 1) issues.push("under-tagged");

  if (isStale(entry.updated_at)) issues.push("stale");

  return issues;
}

const ISSUE_STYLES: Record<QualityIssue, string> = {
  untagged: "bg-destructive/10 dark:bg-destructive/15 text-destructive border-destructive/20 dark:border-destructive/30",
  "under-tagged": "bg-yellow-500/10 dark:bg-yellow-500/15 text-yellow-600 border-yellow-500/20 dark:text-yellow-400 dark:border-yellow-500/30",
  stale: "bg-orange-500/10 dark:bg-orange-500/15 text-orange-600 border-orange-500/20 dark:text-orange-400 dark:border-orange-500/30",
};

export function QualityBadge({
  issue,
  className,
}: {
  issue: QualityIssue;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-medium leading-none",
        ISSUE_STYLES[issue],
        className,
      )}
    >
      {issue}
    </span>
  );
}
