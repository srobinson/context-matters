import { ArrowUpRight } from "lucide-react";
import type { Entry } from "@/api/generated/Entry";
import { KindBadge } from "@/components/domain/KindBadge";
import { getQualityIssues, QualityBadge } from "@/components/domain/QualityBadge";
import { timeAgo } from "@/lib/time";
import { cn } from "@/lib/utils";

interface EntrySummaryProps {
  entry: Entry;
  className?: string;
  showArrow?: boolean;
  showTime?: boolean;
  showScope?: boolean;
  showTags?: boolean;
  showQuality?: boolean;
  interactive?: boolean;
}

function getAgentName(createdBy: string) {
  const parts = createdBy.split(":");
  return parts.length > 1 ? parts.slice(1).join(":") : createdBy;
}

export function EntrySummary({
  entry,
  className,
  showArrow = false,
  showTime = true,
  showScope = true,
  showTags = true,
  showQuality = false,
  interactive = false,
}: EntrySummaryProps) {
  const tags = entry.meta?.tags ?? [];
  const qualityIssues = getQualityIssues(entry);
  const agentName = getAgentName(entry.created_by);

  return (
    <div
      className={cn(
        "group flex items-start gap-3 rounded-control",
        interactive && "transition-colors",
        className,
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-3">
          <p className="line-clamp-2 text-sm font-medium leading-snug text-foreground">
            {entry.title}
          </p>
          {showTime && (
            <time
              dateTime={entry.updated_at}
              className="shrink-0 font-mono text-[10px] text-muted-foreground/70"
              title={new Date(entry.updated_at).toLocaleString()}
            >
              {timeAgo(entry.updated_at)}
            </time>
          )}
        </div>
        <div className="mt-2 mb-1 flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[10px] text-muted-foreground">
          <KindBadge kind={entry.kind} className="shrink-0" />
          <span className="text-muted-foreground/30">/</span>
          <span>{agentName}</span>
          {showScope && (
            <>
              <span className="text-muted-foreground/30">/</span>
              <span className="truncate">{entry.scope_path}</span>
            </>
          )}
          {showTags && tags.length > 0 && (
            <>
              <span className="text-muted-foreground/30">/</span>
              <span className="truncate">
                {tags.slice(0, 2).join(", ")}
                {tags.length > 2 && ` +${tags.length - 2}`}
              </span>
            </>
          )}
          {showQuality &&
            qualityIssues.map((issue) => (
              <QualityBadge key={issue} issue={issue} className="ml-1" />
            ))}
        </div>
      </div>
      {showArrow && (
        <ArrowUpRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground/30 transition-colors group-hover:text-muted-foreground" />
      )}
    </div>
  );
}
