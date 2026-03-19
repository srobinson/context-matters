import { useMemo } from "react";
import { createRoute } from "@tanstack/react-router";
import { rootRoute } from "./__root";
import { useStats } from "@/api/hooks";
import type { Stats } from "@/api/client";
import { StatCard } from "@/components/StatCard";

export const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

function computeQualityScore(stats: Stats): number {
  const total = stats.active_entries;
  if (total === 0) return 100;

  const tagged = total - stats.quality.untagged_count;
  const nonStale = total - stats.quality.stale_count;
  return Math.round((tagged / total) * 50 + (nonStale / total) * 50);
}

function DashboardPage() {
  const { data: stats, isLoading, isError, error } = useStats();

  const qualityScore = useMemo(
    () => (stats ? computeQualityScore(stats) : null),
    [stats],
  );

  const agentSummary = useMemo(() => {
    if (!stats?.active_agents) return null;
    const count = stats.active_agents.length;
    if (count === 0) return "none";
    const names = stats.active_agents
      .slice(0, 3)
      .map((a) => {
        const parts = a.created_by.split(":");
        return parts.length > 1 ? parts.slice(1).join(":") : a.created_by;
      })
      .join(", ");
    if (count > 3) return `${names} +${count - 3}`;
    return names;
  }, [stats]);

  if (isLoading) {
    return (
      <div className="space-y-6">
        <DashboardHeader />
        <div className="grid grid-cols-4 gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div
              key={i}
              className="animate-pulse rounded-lg border border-border bg-card p-4 h-[88px]"
            />
          ))}
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-6">
        <DashboardHeader />
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">
            Failed to load stats: {error.message}
          </p>
        </div>
      </div>
    );
  }

  if (!stats) return null;

  return (
    <div className="space-y-6">
      <DashboardHeader />

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <StatCard
          label="active entries"
          value={stats.active_entries}
          detail={`${stats.superseded_entries} forgotten`}
        />
        <StatCard
          label="today"
          value={stats.entries_today}
          detail={`${stats.entries_this_week} this week`}
        />
        <StatCard
          label="agents"
          value={stats.active_agents.length}
          detail={agentSummary ?? undefined}
        />
        <StatCard
          label="quality"
          value={`${qualityScore}%`}
          detail={qualityDetail(stats)}
        />
      </div>
    </div>
  );
}

function DashboardHeader() {
  return (
    <div className="flex items-baseline gap-3">
      <h2 className="text-lg font-medium tracking-tight">Dashboard</h2>
      <span className="font-mono text-xs text-muted-foreground">
        system overview
      </span>
    </div>
  );
}

function qualityDetail(stats: Stats): string {
  const issues: string[] = [];
  if (stats.quality.untagged_count > 0) {
    issues.push(`${stats.quality.untagged_count} untagged`);
  }
  if (stats.quality.stale_count > 0) {
    issues.push(`${stats.quality.stale_count} stale`);
  }
  if (issues.length === 0) return "all clear";
  return issues.join(", ");
}
