import { useState } from "react";
import { cn } from "@/lib/utils";

export interface RecallTrace {
  routing: string;
  candidates_before_filter: number;
  fetch_limit_used: number;
  token_budget_exhausted: boolean;
}

export interface BrowseTrace {
  filter_set: string[];
  sort: string;
}

interface RecallTraceData {
  kind: "recall";
  trace: RecallTrace;
  scope_chain: string[];
  token_estimate: number;
  returned: number;
}

interface BrowseTraceData {
  kind: "browse";
  trace: BrowseTrace;
  total: number;
  has_more: boolean;
}

export type TraceData = RecallTraceData | BrowseTraceData;

function TraceBadge({ children, variant }: { children: React.ReactNode; variant?: "muted" | "active" | "warn" }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 font-mono text-[10px] leading-none",
        variant === "active" && "border-ring/30 bg-ring/10 text-foreground",
        variant === "warn" && "border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400",
        (!variant || variant === "muted") && "border-border bg-muted text-muted-foreground",
      )}
    >
      {children}
    </span>
  );
}

function TraceRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-2">
      <span className="shrink-0 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        {label}
      </span>
      <div className="flex flex-wrap items-center gap-1">{children}</div>
    </div>
  );
}

function RecallTraceContent({ data }: { data: RecallTraceData }) {
  return (
    <div className="space-y-1.5">
      <TraceRow label="routing">
        <TraceBadge variant="active">{data.trace.routing}</TraceBadge>
      </TraceRow>
      <TraceRow label="returned">
        <span className="font-mono text-xs text-muted-foreground">{data.returned}</span>
        <span className="font-mono text-[10px] text-muted-foreground/50">
          of {data.trace.candidates_before_filter} candidates
        </span>
      </TraceRow>
      <TraceRow label="fetch limit">
        <span className="font-mono text-xs text-muted-foreground">{data.trace.fetch_limit_used}</span>
      </TraceRow>
      <TraceRow label="token estimate">
        <span className="font-mono text-xs text-muted-foreground">{data.token_estimate.toLocaleString()}</span>
        {data.trace.token_budget_exhausted && (
          <TraceBadge variant="warn">budget exhausted</TraceBadge>
        )}
      </TraceRow>
      {data.scope_chain.length > 0 && (
        <TraceRow label="scope chain">
          <span className="font-mono text-xs text-muted-foreground">
            {data.scope_chain.join(" > ")}
          </span>
        </TraceRow>
      )}
    </div>
  );
}

function BrowseTraceContent({ data }: { data: BrowseTraceData }) {
  return (
    <div className="space-y-1.5">
      <TraceRow label="sort">
        <TraceBadge variant="active">{data.trace.sort}</TraceBadge>
      </TraceRow>
      <TraceRow label="total">
        <span className="font-mono text-xs text-muted-foreground">{data.total}</span>
        {data.has_more && (
          <TraceBadge variant="muted">has more</TraceBadge>
        )}
      </TraceRow>
      {data.trace.filter_set.length > 0 && (
        <TraceRow label="filters">
          {data.trace.filter_set.map((f) => (
            <TraceBadge key={f}>{f}</TraceBadge>
          ))}
        </TraceRow>
      )}
      {data.trace.filter_set.length === 0 && (
        <TraceRow label="filters">
          <span className="font-mono text-[10px] text-muted-foreground/50">none</span>
        </TraceRow>
      )}
    </div>
  );
}

export function TracePanel({ data }: { data: TraceData }) {
  const [open, setOpen] = useState(false);

  return (
    <div className="rounded-md border border-dashed border-border/60 bg-muted/30">
      <button
        type="button"
        onClick={() => setOpen((p) => !p)}
        className="flex w-full items-center justify-between px-3 py-1.5 font-mono text-[10px] text-muted-foreground/70 transition-colors hover:text-muted-foreground"
      >
        <span className="uppercase tracking-wider">
          {data.kind} trace
        </span>
        <span>{open ? "\u25B4" : "\u25BE"}</span>
      </button>
      {open && (
        <div className="border-t border-dashed border-border/40 px-3 py-2">
          {data.kind === "recall" ? (
            <RecallTraceContent data={data} />
          ) : (
            <BrowseTraceContent data={data} />
          )}
        </div>
      )}
    </div>
  );
}
