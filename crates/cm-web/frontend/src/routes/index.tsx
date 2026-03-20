import { createRoute, Link } from "@tanstack/react-router";
import { ArrowRight, Download, Sparkles } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { toast } from "sonner";
import { api, type Stats } from "@/api/client";
import { useStats } from "@/api/hooks";
import { StatCard } from "@/components/composed/StatCard";
import { RecentActivity } from "@/components/RecentActivity";
import { ScopeTree } from "@/components/ScopeTree";
import { rootRoute } from "./__root";

export const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

interface ReviewSignal {
  label: string;
  count: number;
  to: "/feed";
  search: Record<string, string | boolean>;
  description: string;
}

function computeQualityScore(stats: Stats): number {
  const total = stats.active_entries;
  if (total === 0) return 100;

  const tagged = total - stats.quality.untagged_count;
  const nonStale = total - stats.quality.stale_count;
  const scoped = total - stats.quality.global_scope_count;
  return Math.round((tagged / total) * 34 + (nonStale / total) * 33 + (scoped / total) * 33);
}

function buildReviewSignals(stats: Stats): ReviewSignal[] {
  const q = stats.quality;
  const signals: ReviewSignal[] = [];

  if (q.global_scope_count > 0) {
    signals.push({
      label: "global scope",
      count: q.global_scope_count,
      to: "/feed",
      search: { scope_path: "global" },
      description: "Entries still living at the broadest scope",
    });
  }

  if (q.stale_count > 0) {
    signals.push({
      label: "stale entries",
      count: q.stale_count,
      to: "/feed",
      search: { sort: "oldest" },
      description: "Older entries that likely need review or replacement",
    });
  }

  if (q.untagged_count > 0) {
    signals.push({
      label: "untagged entries",
      count: q.untagged_count,
      to: "/feed",
      search: { tag: "__untagged__" },
      description: "Entries missing tagging support",
    });
  }

  return signals.sort((a, b) => b.count - a.count);
}

function getHotScopes(stats: Stats): { path: string; count: number }[] {
  return [...stats.scope_tree]
    .sort((a, b) => b.entry_count - a.entry_count)
    .slice(0, 4)
    .map((scope) => ({ path: scope.path, count: scope.entry_count }));
}

function DashboardPage() {
  const { data: stats, isLoading, isError, error } = useStats();

  const qualityScore = useMemo(() => (stats ? computeQualityScore(stats) : null), [stats]);

  const agentSummary = useMemo(() => {
    if (!stats?.active_agents) return null;

    const count = stats.active_agents.length;
    if (count === 0) return "none";

    const names = stats.active_agents
      .slice(0, 3)
      .map((agent) => {
        const parts = agent.created_by.split(":");
        return parts.length > 1 ? parts.slice(1).join(":") : agent.created_by;
      })
      .join(", ");

    return count > 3 ? `${names} +${count - 3}` : names;
  }, [stats]);

  const reviewSignals = useMemo(() => (stats ? buildReviewSignals(stats) : []), [stats]);

  const hotScopes = useMemo(() => (stats ? getHotScopes(stats) : []), [stats]);

  const primarySignal = reviewSignals[0];

  if (isLoading) {
    return (
      <div className="space-y-6">
        <DashboardHeader />
        <div className="grid gap-4 lg:grid-cols-[minmax(0,1.6fr)_minmax(20rem,1fr)]">
          <div className="h-[228px] animate-pulse rounded-surface border border-border/70 bg-card/70 shadow-surface" />
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-1">
            {Array.from({ length: 3 }).map((_, index) => (
              <div
                key={index}
                className="h-[88px] animate-pulse rounded-surface border border-border/70 bg-card/70 shadow-surface"
              />
            ))}
          </div>
        </div>
        <div className="grid gap-6 lg:grid-cols-5">
          <div className="lg:col-span-3 h-[420px] animate-pulse rounded-surface border border-border/70 bg-card/70 shadow-surface" />
          <div className="lg:col-span-2 h-[420px] animate-pulse rounded-surface border border-border/70 bg-card/70 shadow-surface" />
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-6">
        <DashboardHeader />
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
          <p className="text-sm text-destructive">Failed to load stats: {error.message}</p>
        </div>
      </div>
    );
  }

  if (!stats) return null;

  return (
    <div className="space-y-6">
      <DashboardHeader />

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1.6fr)_minmax(20rem,1fr)]">
        <ReviewPanel
          primarySignal={primarySignal}
          reviewSignals={reviewSignals}
          hotScopes={hotScopes}
        />

        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-1">
          <StatCard
            label="review health"
            value={`${qualityScore}%`}
            detail={qualityDetail(stats)}
            className="border-border/70 bg-card/70 p-5"
          />
          <StatCard
            label="active entries"
            value={stats.active_entries}
            detail={`${stats.superseded_entries} forgotten`}
            className="border-border/70 bg-card/70 p-5"
          />
          <div className="grid grid-cols-2 gap-4">
            <StatCard
              label="today"
              value={stats.entries_today}
              detail={`${stats.entries_this_week} this week`}
              className="border-border/70 bg-card/70 p-5"
            />
            <StatCard
              label="agents active"
              value={stats.active_agents.length}
              detail={agentSummary ?? undefined}
              className="border-border/70 bg-card/70 p-5"
            />
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-5">
        <div className="lg:col-span-3">
          <RecentActivity />
        </div>
        <div className="lg:col-span-2">
          <ScopeTree stats={stats} />
        </div>
      </div>
    </div>
  );
}

function DashboardHeader() {
  const [isExporting, setIsExporting] = useState(false);

  const handleExport = useCallback(async () => {
    setIsExporting(true);
    try {
      const blob = await api.export();
      const timestamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `cm-export-${timestamp}.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast.success("Export downloaded");
    } catch {
      toast.error("Export failed");
    } finally {
      setIsExporting(false);
    }
  }, []);

  return (
    <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
      <div className="space-y-2">
        <div className="flex items-baseline gap-3">
          <h2 className="text-3xl font-medium tracking-tight">Dashboard</h2>
          <span className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground/70">
            system health
          </span>
        </div>
        <p className="max-w-2xl text-sm text-muted-foreground">
          Review pressure, scope concentration, and recent changes across the context store.
        </p>
      </div>

      <button
        type="button"
        onClick={handleExport}
        disabled={isExporting}
        className="inline-flex items-center gap-1.5 self-start rounded-control border border-border bg-muted px-3 py-2 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
      >
        <Download className="h-3.5 w-3.5" />
        {isExporting ? "exporting..." : "export"}
      </button>
    </div>
  );
}

function ReviewPanel({
  primarySignal,
  reviewSignals,
  hotScopes,
}: {
  primarySignal?: ReviewSignal;
  reviewSignals: ReviewSignal[];
  hotScopes: { path: string; count: number }[];
}) {
  const topScope = hotScopes[0];

  if (!primarySignal) {
    return (
      <section className="rounded-surface border border-emerald-500/25 bg-emerald-500/8 p-4 shadow-surface">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-3">
            <div className="inline-flex items-center gap-2 rounded-chip border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1 font-mono text-[11px] uppercase tracking-[0.24em] text-emerald-300">
              <Sparkles className="h-3.5 w-3.5" />
              all clear
            </div>
            <div className="space-y-1">
              <h3 className="text-2xl font-medium tracking-tight text-foreground">
                No review backlog detected
              </h3>
              <p className="max-w-xl text-sm text-muted-foreground">
                Quality checks are currently clean. Use the feed to inspect recent additions or
                export the store snapshot.
              </p>
            </div>
          </div>

          <Link
            to="/feed"
            search={{ sort: "recent" }}
            className="inline-flex items-center gap-1.5 rounded-control border border-border/80 bg-background/70 px-3 py-2 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            inspect feed
            <ArrowRight className="h-3.5 w-3.5" />
          </Link>
        </div>
      </section>
    );
  }

  return (
    <section className="rounded-surface border border-amber-500/25 bg-[linear-gradient(135deg,rgba(120,77,0,0.12),rgba(0,0,0,0))] p-4 shadow-surface">
      <div className="grid gap-5 lg:grid-cols-[minmax(0,1.3fr)_minmax(16rem,1fr)]">
        <div className="space-y-4">
          <div className="space-y-3">
            <div className="inline-flex items-center gap-2 rounded-chip border border-amber-500/25 bg-amber-500/8 px-2.5 py-1 font-mono text-[11px] uppercase tracking-[0.24em] text-amber-300">
              <Sparkles className="h-3.5 w-3.5" />
              needs review
            </div>
            <div className="space-y-1">
              <div className="flex flex-wrap items-end gap-x-3 gap-y-1">
                <span className="font-mono text-5xl font-semibold leading-none tracking-tight text-foreground">
                  {primarySignal.count}
                </span>
                <div className="pb-1 font-mono text-sm text-amber-100/80">
                  {primarySignal.label}
                </div>
              </div>
              <p className="max-w-xl text-sm text-muted-foreground">
                {primarySignal.description}. This is the largest review queue surfaced by the
                current heuristics.
              </p>
            </div>
          </div>

          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <Link
                to={primarySignal.to}
                search={primarySignal.search}
                className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-control border border-amber-500/25 bg-amber-500/10 px-3 py-2 font-mono text-xs text-amber-100 transition-colors hover:bg-amber-500/15"
              >
                open review queue
                <ArrowRight className="h-3.5 w-3.5" />
              </Link>
            </div>

            <p className="font-mono text-[10px] uppercase tracking-[0.24em] text-amber-100/60">
              review signals
            </p>
            <div className="flex flex-wrap gap-2">
              {reviewSignals.map((signal) => (
                <Link
                  key={signal.label}
                  to={signal.to}
                  search={signal.search}
                  className="inline-flex items-center gap-2 rounded-control border border-amber-500/20 bg-background/30 px-3 py-2 font-mono text-xs text-foreground transition-colors hover:bg-background/45"
                >
                  <span className="text-amber-200">{signal.count}</span>
                  <span className="text-muted-foreground">{signal.label}</span>
                </Link>
              ))}
            </div>
          </div>
        </div>

        <div className="space-y-3">
          <p className="font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/60">
            concentration
          </p>
          {topScope ? (
            <p className="text-sm text-muted-foreground">
              Heaviest scope: <span className="font-mono text-foreground">{topScope.path}</span>{" "}
              with {topScope.count} entries.
            </p>
          ) : (
            <p className="text-sm text-muted-foreground">Scope load data is not available yet.</p>
          )}

          <div className="space-y-2">
            {hotScopes.slice(0, 3).map((scope) => (
              <Link
                key={scope.path}
                to="/feed"
                search={{ scope_path: scope.path }}
                className="flex items-center justify-between rounded-control border border-border/60 bg-background/35 px-3 py-2 font-mono text-xs transition-colors hover:bg-accent/20"
              >
                <span className="truncate text-foreground">{scope.path}</span>
                <span className="ml-3 shrink-0 text-muted-foreground">{scope.count}</span>
              </Link>
            ))}
          </div>
        </div>
      </div>
    </section>
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

  if (stats.quality.global_scope_count > 0) {
    issues.push(`${stats.quality.global_scope_count} broad scope`);
  }

  return issues.length === 0 ? "no active review signals" : issues.join(", ");
}
