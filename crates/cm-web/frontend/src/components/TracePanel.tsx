import { useState } from "react";
import type { WebBrowseHeader } from "@/api/generated/WebBrowseHeader";
import type { WebRecallHeader } from "@/api/generated/WebRecallHeader";
import { cn } from "@/lib/utils";

interface RecallHeaderData {
  kind: "recall";
  header: WebRecallHeader;
  advisories: string[];
}

interface BrowseHeaderData {
  kind: "browse";
  header: Omit<WebBrowseHeader, "total"> & { total: number };
  has_more: boolean;
}

export type TraceData = RecallHeaderData | BrowseHeaderData;

function TraceBadge({
  children,
  variant,
}: {
  children: React.ReactNode;
  variant?: "muted" | "active" | "warn";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 font-mono text-[10px] leading-none",
        variant === "active" && "border-ring/30 bg-ring/10 text-foreground",
        variant === "warn" &&
          "border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400",
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

function HistogramRow({ label, histogram }: { label: string; histogram: Record<string, number> }) {
  const entries = Object.entries(histogram)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 6);
  if (entries.length === 0) return null;
  return (
    <TraceRow label={label}>
      {entries.map(([name, count]) => (
        <TraceBadge key={name}>
          {name} {count}
        </TraceBadge>
      ))}
    </TraceRow>
  );
}

function RecallTraceContent({ data }: { data: RecallHeaderData }) {
  const { header, advisories } = data;
  return (
    <div className="space-y-1.5">
      <TraceRow label="routing">
        <TraceBadge variant="active">{header.routing}</TraceBadge>
        {header.tier && <TraceBadge variant="muted">tier: {header.tier}</TraceBadge>}
      </TraceRow>
      <TraceRow label="returned">
        <span className="font-mono text-xs text-muted-foreground">{header.returned}</span>
        <span className="font-mono text-[10px] text-muted-foreground/50">
          of {header.candidates} candidates
        </span>
      </TraceRow>
      <TraceRow label="tokens">
        <span className="font-mono text-xs text-muted-foreground">
          {header.tokens.toLocaleString()}
        </span>
      </TraceRow>
      {header.scope_chain.length > 0 && (
        <TraceRow label="scope chain">
          <span className="font-mono text-xs text-muted-foreground">
            {header.scope_chain.join(" > ")}
          </span>
        </TraceRow>
      )}
      <HistogramRow label="kinds" histogram={header.kinds_histogram} />
      <HistogramRow label="tags" histogram={header.tags_histogram} />
      {advisories.length > 0 && (
        <TraceRow label="advisories">
          {advisories.map((msg) => (
            <TraceBadge key={msg} variant="warn">
              {msg}
            </TraceBadge>
          ))}
        </TraceRow>
      )}
    </div>
  );
}

function BrowseTraceContent({ data }: { data: BrowseHeaderData }) {
  const { header, has_more } = data;
  return (
    <div className="space-y-1.5">
      <TraceRow label="sort">
        <TraceBadge variant="active">{header.sort_used}</TraceBadge>
      </TraceRow>
      <TraceRow label="total">
        <span className="font-mono text-xs text-muted-foreground">{header.total}</span>
        <span className="font-mono text-[10px] text-muted-foreground/50">
          ({header.returned} returned)
        </span>
        {has_more && <TraceBadge variant="muted">has more</TraceBadge>}
      </TraceRow>
      <HistogramRow label="kinds" histogram={header.kinds_histogram} />
      <HistogramRow label="tags" histogram={header.tags_histogram} />
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
        <span className="uppercase tracking-wider">{data.kind} trace</span>
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
