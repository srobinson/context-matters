import { Link } from "@tanstack/react-router";
import { ArrowUpRight } from "lucide-react";
import { useMemo } from "react";
import type { WebBrowseRow } from "@/api/generated/WebBrowseRow";
import { useEntries } from "@/api/hooks";
import { EntrySummary } from "./composed/EntrySummary";
import { HoistedHeader } from "./composed/HoistedHeader";

export function RecentActivity() {
  const { data, isLoading } = useEntries({ sort: "recent", limit: 8 });

  const entries = useMemo<WebBrowseRow[]>(
    () => data?.pages.flatMap((page) => page.entries) ?? [],
    [data],
  );
  const header = data?.pages[0]?.header;

  return (
    <section className="rounded-surface border border-border/70 bg-card/70 p-4 shadow-surface backdrop-blur-sm">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div className="space-y-1">
          <h3 className="font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/60">
            recent activity
          </h3>
          <p className="text-sm text-muted-foreground">
            Latest writes across the store. Use this to spot what changed and who touched it.
          </p>
        </div>
        <Link
          to="/feed"
          search={{ sort: "recent" as const }}
          className="inline-flex items-center gap-1 rounded-control border border-border/80 bg-background/70 px-2.5 py-1.5 font-mono text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          open feed
          <ArrowUpRight className="h-3 w-3" />
        </Link>
      </div>

      {isLoading && (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-16 animate-pulse rounded-control border border-border/60 bg-background/60"
            />
          ))}
        </div>
      )}

      {!isLoading && entries.length === 0 && (
        <p className="font-mono text-xs text-muted-foreground">No entries yet.</p>
      )}

      {entries.length > 0 && (
        <div className="space-y-2">
          {header && (
            <HoistedHeader scope={header.scope} kind={header.kind} createdBy={header.created_by} />
          )}
          {entries.map((row) => (
            <ActivityRow key={row.id} row={row} />
          ))}
        </div>
      )}
    </section>
  );
}

function ActivityRow({ row }: { row: WebBrowseRow }) {
  return (
    <Link
      to="/feed"
      search={{ sort: "recent" as const, entry_id: row.id }}
      className="group flex items-start gap-3 rounded-control border border-border/60 bg-background/40 px-3 py-3 transition-colors hover:border-border hover:bg-accent/20"
    >
      <EntrySummary row={row} showArrow />
    </Link>
  );
}
