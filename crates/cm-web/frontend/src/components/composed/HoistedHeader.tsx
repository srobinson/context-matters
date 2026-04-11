import type { EntryKind } from "@/api/generated/EntryKind";
import { KindBadge } from "@/components/domain/KindBadge";
import { cn } from "@/lib/utils";

interface HoistedHeaderProps {
  scope?: string | null;
  kind?: string | null;
  createdBy?: string | null;
  className?: string;
}

/**
 * Strips the `source_type:` prefix from a `created_by` attribution string,
 * e.g. `"agent:claude-code"` becomes `"claude-code"`. When there is no
 * colon, returns the input unchanged.
 */
export function getAgentName(createdBy: string): string {
  const parts = createdBy.split(":");
  return parts.length > 1 ? parts.slice(1).join(":") : createdBy;
}

/**
 * Above-list chip strip surfacing the constants that a browse view has
 * hoisted out of every row (shared `scope`, `kind`, `created_by`). Renders
 * `null` when all fields are absent, so callers can drop it in front of
 * any list unconditionally.
 *
 * Placing this strip above the row list gives the reader a one-glance view
 * of what the whole list shares, while each row below only renders its
 * per-row variations. Paired with the row components dropping those same
 * columns when they fall back to header values.
 */
export function HoistedHeader({ scope, kind, createdBy, className }: HoistedHeaderProps) {
  if (!scope && !kind && !createdBy) return null;

  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border border-border/60 bg-muted/30 px-3 py-1.5 font-mono text-[11px]",
        className,
      )}
    >
      <span className="uppercase tracking-wider text-muted-foreground/60">shared</span>
      {kind && (
        <div className="flex items-center gap-1.5">
          <span className="text-muted-foreground/50">kind</span>
          <KindBadge kind={kind as EntryKind} />
        </div>
      )}
      {scope && (
        <div className="flex items-center gap-1.5">
          <span className="text-muted-foreground/50">scope</span>
          <span className="truncate text-muted-foreground">{scope}</span>
        </div>
      )}
      {createdBy && (
        <div className="flex items-center gap-1.5">
          <span className="text-muted-foreground/50">by</span>
          <span className="text-muted-foreground">{getAgentName(createdBy)}</span>
        </div>
      )}
    </div>
  );
}
