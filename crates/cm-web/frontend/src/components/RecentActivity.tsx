import { Link } from "@tanstack/react-router";
import type { Entry } from "@/api/generated/Entry";
import { useEntries } from "@/api/hooks";
import { KindBadge } from "./domain/KindBadge";
import { timeAgo } from "@/lib/time";

export function RecentActivity() {
  const { data, isLoading } = useEntries({ sort: "recent", limit: 20 });

  const entries = data?.pages.flatMap((page) => page.items) ?? [];

  return (
    <div className="space-y-3">
      <h3 className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        recent activity
      </h3>

      {isLoading && (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="animate-pulse rounded-md border border-border bg-card h-12"
            />
          ))}
        </div>
      )}

      {!isLoading && entries.length === 0 && (
        <p className="font-mono text-xs text-muted-foreground">
          No entries yet.
        </p>
      )}

      {entries.length > 0 && (
        <div className="space-y-1">
          {entries.map((entry) => (
            <ActivityRow key={entry.id} entry={entry} />
          ))}
        </div>
      )}
    </div>
  );
}

function ActivityRow({ entry }: { entry: Entry }) {
  const agentParts = entry.created_by.split(":");
  const agentName =
    agentParts.length > 1 ? agentParts.slice(1).join(":") : entry.created_by;
  const tags = entry.meta?.tags ?? [];

  return (
    <Link
      to="/feed"
      search={{ sort: "recent" as const }}
      className="flex items-center gap-2.5 rounded-md border border-transparent px-2 py-1.5 transition-colors hover:border-border hover:bg-accent/30"
    >
      <KindBadge kind={entry.kind} />
      <div className="min-w-0 flex-1">
        <p className="truncate text-xs font-medium text-foreground">
          {entry.title}
        </p>
        <div className="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground">
          <span>{agentName}</span>
          <span className="text-muted-foreground/30">/</span>
          <span className="truncate">{entry.scope_path}</span>
          {tags.length > 0 && (
            <>
              <span className="text-muted-foreground/30">/</span>
              <span className="truncate">
                {tags.slice(0, 2).join(", ")}
                {tags.length > 2 && ` +${tags.length - 2}`}
              </span>
            </>
          )}
        </div>
      </div>
      <time
        dateTime={entry.updated_at}
        className="shrink-0 font-mono text-[10px] text-muted-foreground/60"
        title={new Date(entry.updated_at).toLocaleString()}
      >
        {timeAgo(entry.updated_at)}
      </time>
    </Link>
  );
}
