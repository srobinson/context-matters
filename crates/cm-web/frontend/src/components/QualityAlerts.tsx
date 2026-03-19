import { useMemo } from "react";
import { Link } from "@tanstack/react-router";
import type { Stats } from "@/api/client";
import { TriangleAlert } from "lucide-react";

interface QualityAlert {
  label: string;
  count: number;
  to: string;
  search: Record<string, string | boolean>;
}

function buildAlerts(stats: Stats): QualityAlert[] {
  const alerts: QualityAlert[] = [];
  const q = stats.quality;

  if (q.untagged_count > 0) {
    alerts.push({
      label: "untagged",
      count: q.untagged_count,
      to: "/feed",
      search: { tag: "__untagged__" },
    });
  }

  if (q.stale_count > 0) {
    alerts.push({
      label: "stale",
      count: q.stale_count,
      to: "/feed",
      search: { sort: "oldest" },
    });
  }

  if (q.global_scope_count > 0) {
    alerts.push({
      label: "at global scope",
      count: q.global_scope_count,
      to: "/feed",
      search: { scope_path: "global" },
    });
  }

  return alerts;
}

export function QualityAlerts({ stats }: { stats: Stats }) {
  const alerts = useMemo(() => buildAlerts(stats), [stats]);

  if (alerts.length === 0) return null;

  const totalCount = alerts.reduce((sum, a) => sum + a.count, 0);

  return (
    <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 flex items-start gap-2.5">
      <TriangleAlert className="h-4 w-4 shrink-0 text-amber-500 mt-0.5" />
      <div className="font-mono text-xs text-foreground">
        <span className="font-medium">{totalCount} entries need attention</span>
        <span className="text-muted-foreground">
          {": "}
          {alerts.map((alert, i) => (
            <span key={alert.label}>
              {i > 0 && ", "}
              <Link
                to={alert.to}
                search={alert.search}
                className="underline underline-offset-2 decoration-muted-foreground/40 hover:text-foreground hover:decoration-foreground transition-colors"
              >
                {alert.count} {alert.label}
              </Link>
            </span>
          ))}
        </span>
      </div>
    </div>
  );
}
