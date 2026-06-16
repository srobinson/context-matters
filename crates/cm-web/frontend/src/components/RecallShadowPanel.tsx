import { Link } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import type { RecallShadowRow } from "@/api/generated/RecallShadowRow";
import { useRecallShadowHistory } from "@/api/hooks";
import { timeAgo } from "@/lib/time";

type ChangeFilter = "" | "changed" | "unchanged";

interface RecallShadowSummary {
  total: number;
  divergenceRate: number;
  averageOverlap: number;
}

function summarizeRows(rows: RecallShadowRow[]): RecallShadowSummary {
  if (rows.length === 0) {
    return { total: 0, divergenceRate: 0, averageOverlap: 0 };
  }

  const changed = rows.filter((row) => row.top1_changed).length;
  const overlap = rows.reduce((sum, row) => sum + row.topk_overlap, 0) / rows.length;

  return {
    total: rows.length,
    divergenceRate: changed / rows.length,
    averageOverlap: overlap,
  };
}

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

export function RecallShadowPanel() {
  const [routing, setRouting] = useState("");
  const [scopePath, setScopePath] = useState("");
  const [changeFilter, setChangeFilter] = useState<ChangeFilter>("");

  const params = useMemo(
    () => ({
      limit: 200,
      routing: routing.trim() || undefined,
      scope_path: scopePath.trim() || undefined,
      top1_changed: changeFilter === "" ? undefined : changeFilter === "changed",
    }),
    [routing, scopePath, changeFilter],
  );

  const { data, isLoading } = useRecallShadowHistory(params);
  const rows = data ?? [];
  const recentRows = useMemo(() => rows.slice(0, 8), [rows]);
  const summary = useMemo(() => summarizeRows(rows), [rows]);

  return (
    <section className="rounded-surface border border-border/70 bg-card/70 p-4 shadow-surface backdrop-blur-sm">
      <div className="mb-4 flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-1">
          <h3 className="font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/60">
            recall shadow
          </h3>
          <p className="max-w-2xl text-sm text-muted-foreground">
            Canary diff rows for legacy versus ranked recall ordering.
          </p>
        </div>
        <RecallShadowFilters
          routing={routing}
          scopePath={scopePath}
          changeFilter={changeFilter}
          onRoutingChange={setRouting}
          onScopePathChange={setScopePath}
          onChangeFilterChange={setChangeFilter}
        />
      </div>

      <div className="mb-4 grid gap-3 sm:grid-cols-3">
        <Metric label="divergence" value={formatPercent(summary.divergenceRate)} />
        <Metric label="avg overlap" value={formatPercent(summary.averageOverlap)} />
        <Metric label="total rows" value={summary.total.toLocaleString()} />
      </div>

      {isLoading && <RecallShadowLoading />}

      {!isLoading && rows.length === 0 && (
        <p className="rounded-control border border-border/60 bg-background/40 p-4 font-mono text-xs text-muted-foreground">
          No recall shadow rows yet. Run recall in shadow or live mode to populate the canary.
        </p>
      )}

      {rows.length > 0 && (
        <div className="space-y-2">
          {recentRows.map((row) => (
            <RecallShadowRowView key={row.id} row={row} />
          ))}
        </div>
      )}
    </section>
  );
}

function RecallShadowFilters({
  routing,
  scopePath,
  changeFilter,
  onRoutingChange,
  onScopePathChange,
  onChangeFilterChange,
}: {
  routing: string;
  scopePath: string;
  changeFilter: ChangeFilter;
  onRoutingChange: (value: string) => void;
  onScopePathChange: (value: string) => void;
  onChangeFilterChange: (value: ChangeFilter) => void;
}) {
  return (
    <div className="grid gap-2 sm:grid-cols-3 lg:min-w-[34rem]">
      <input
        value={routing}
        onChange={(event) => onRoutingChange(event.target.value)}
        placeholder="routing"
        className="rounded-control border border-border/70 bg-background/70 px-3 py-2 font-mono text-xs outline-none transition-colors placeholder:text-muted-foreground/50 focus:border-foreground/50"
      />
      <input
        value={scopePath}
        onChange={(event) => onScopePathChange(event.target.value)}
        placeholder="scope path"
        className="rounded-control border border-border/70 bg-background/70 px-3 py-2 font-mono text-xs outline-none transition-colors placeholder:text-muted-foreground/50 focus:border-foreground/50"
      />
      <select
        value={changeFilter}
        onChange={(event) => onChangeFilterChange(event.target.value as ChangeFilter)}
        className="rounded-control border border-border/70 bg-background/70 px-3 py-2 font-mono text-xs outline-none transition-colors focus:border-foreground/50"
      >
        <option value="">all diffs</option>
        <option value="changed">top 1 changed</option>
        <option value="unchanged">top 1 stable</option>
      </select>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-control border border-border/60 bg-background/40 p-3">
      <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground/60">
        {label}
      </p>
      <p className="mt-1 text-lg font-semibold tabular-nums">{value}</p>
    </div>
  );
}

function RecallShadowLoading() {
  return (
    <div className="space-y-2">
      {Array.from({ length: 3 }).map((_, index) => (
        <div
          key={index}
          className="h-24 animate-pulse rounded-control border border-border/60 bg-background/60"
        />
      ))}
    </div>
  );
}

function RecallShadowRowView({ row }: { row: RecallShadowRow }) {
  return (
    <article className="rounded-control border border-border/60 bg-background/40 px-3 py-3">
      <div className="mb-3 flex flex-wrap items-center gap-2 font-mono text-[10px] text-muted-foreground/70">
        <span className="rounded bg-muted px-1.5 py-0.5 text-foreground">{row.routing}</span>
        {row.scope_path && <span className="truncate">{row.scope_path}</span>}
        <span>{formatPercent(row.topk_overlap)} overlap</span>
        <span>{row.top1_changed ? "top 1 changed" : "top 1 stable"}</span>
        <time dateTime={row.ts} title={new Date(row.ts).toLocaleString()}>
          {timeAgo(row.ts)}
        </time>
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        <IdList label="old top k" ids={row.old_ids} />
        <IdList label="new top k" ids={row.new_ids} />
      </div>
    </article>
  );
}

function IdList({ label, ids }: { label: string; ids: string[] }) {
  return (
    <div className="min-w-0">
      <p className="mb-1 font-mono text-[10px] uppercase tracking-[0.16em] text-muted-foreground/60">
        {label}
      </p>
      {ids.length === 0 ? (
        <p className="font-mono text-xs text-muted-foreground">none</p>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {ids.map((id) => (
            <Link
              key={id}
              to="/feed"
              search={{ sort: "recent" as const, entry_id: id }}
              className="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground transition-colors hover:text-foreground"
              title={id}
            >
              {id.slice(0, 8)}
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
