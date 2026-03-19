import { cn } from "@/lib/utils";

interface StatCardProps {
  label: string;
  value: string | number;
  detail?: string;
  className?: string;
}

export function StatCard({ label, value, detail, className }: StatCardProps) {
  return (
    <div
      className={cn(
        "rounded-surface border border-border bg-card p-4 space-y-1 shadow-surface",
        className,
      )}
    >
      <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        {label}
      </p>
      <p className="font-mono text-2xl font-semibold tracking-tight text-card-foreground">
        {value}
      </p>
      {detail && (
        <p className="font-mono text-[11px] text-muted-foreground">{detail}</p>
      )}
    </div>
  );
}
