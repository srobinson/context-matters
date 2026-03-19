import { useMemo } from "react";

// --- Public types ---

export interface FieldChange {
  label: string;
  original: string;
  current: string;
}

export interface DiffViewProps {
  /** List of changed fields to display */
  changes: FieldChange[];
  /** Called when user confirms */
  onConfirm: () => void;
  /** Called when user wants to go back */
  onBack: () => void;
  /** Whether the confirm action is pending */
  isPending?: boolean;
  /** Error message to display */
  error?: string | null;
  /** Label for the confirm button. Default: "confirm save" */
  confirmLabel?: string;
  /** Label for the back button. Default: "back to editing" */
  backLabel?: string;
  /** Pending label for the confirm button. Default: "saving..." */
  pendingLabel?: string;
}

// --- Main component ---

export function DiffView({
  changes,
  onConfirm,
  onBack,
  isPending = false,
  error,
  confirmLabel = "confirm save",
  backLabel = "back to editing",
  pendingLabel = "saving...",
}: DiffViewProps) {
  if (changes.length === 0) return null;

  return (
    <div className="space-y-3 border-t border-border pt-3">
      <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        review changes
      </p>

      <div className="space-y-2">
        {changes.map((change) => (
          <FieldDiffRow key={change.label} change={change} />
        ))}
      </div>

      <div className="flex items-center gap-2 pt-2">
        <button
          type="button"
          onClick={onConfirm}
          disabled={isPending}
          className="rounded-md border border-border bg-foreground px-3 py-1.5 font-mono text-xs text-background transition-colors hover:bg-foreground/90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isPending ? pendingLabel : confirmLabel}
        </button>
        <button
          type="button"
          onClick={onBack}
          disabled={isPending}
          className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
        >
          {backLabel}
        </button>
        {error && (
          <span className="font-mono text-xs text-destructive">{error}</span>
        )}
      </div>
    </div>
  );
}

// --- Internal components ---

function FieldDiffRow({ change }: { change: FieldChange }) {
  const isMultiline =
    change.original.includes("\n") || change.current.includes("\n");

  return (
    <div className="rounded-lg border border-border bg-muted/30 p-2.5 space-y-1.5">
      <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        {change.label}
      </span>
      {isMultiline ? (
        <MultilineDiff original={change.original} current={change.current} />
      ) : (
        <InlineDiff original={change.original} current={change.current} />
      )}
    </div>
  );
}

function InlineDiff({
  original,
  current,
}: {
  original: string;
  current: string;
}) {
  return (
    <div className="flex items-baseline gap-2 font-mono text-xs">
      <span className="rounded bg-red-500/10 px-1.5 py-0.5 text-red-600 line-through dark:text-red-400">
        {original}
      </span>
      <span className="text-muted-foreground/40">{"\u2192"}</span>
      <span className="rounded bg-green-500/10 px-1.5 py-0.5 text-green-600 dark:text-green-400">
        {current}
      </span>
    </div>
  );
}

function MultilineDiff({
  original,
  current,
}: {
  original: string;
  current: string;
}) {
  const { removed, added, unchanged } = useMemo(
    () => diffLines(original, current),
    [original, current],
  );

  return (
    <div className="space-y-1">
      <pre className="overflow-x-auto rounded-md border border-border bg-muted/50 p-2 font-mono text-[11px] leading-relaxed whitespace-pre-wrap">
        {removed.map((line, i) => (
          <div
            key={`r-${i}`}
            className="bg-red-500/10 text-red-600 dark:text-red-400"
          >
            <span className="select-none text-muted-foreground/40 mr-1">
              -
            </span>
            {line}
          </div>
        ))}
        {unchanged.map((line, i) => (
          <div key={`u-${i}`} className="text-muted-foreground/60">
            <span className="select-none text-muted-foreground/20 mr-1">
              {" "}
            </span>
            {line}
          </div>
        ))}
        {added.map((line, i) => (
          <div
            key={`a-${i}`}
            className="bg-green-500/10 text-green-600 dark:text-green-400"
          >
            <span className="select-none text-muted-foreground/40 mr-1">
              +
            </span>
            {line}
          </div>
        ))}
      </pre>
    </div>
  );
}

/** Simple line-level diff: partition into removed, unchanged, and added lines. */
function diffLines(
  original: string,
  current: string,
): { removed: string[]; added: string[]; unchanged: string[] } {
  const origLines = original.split("\n");
  const currLines = current.split("\n");
  const origSet = new Set(origLines);
  const currSet = new Set(currLines);

  const removed: string[] = [];
  const unchanged: string[] = [];
  const added: string[] = [];

  for (const line of origLines) {
    if (currSet.has(line)) {
      unchanged.push(line);
    } else {
      removed.push(line);
    }
  }

  for (const line of currLines) {
    if (!origSet.has(line)) {
      added.push(line);
    }
  }

  return { removed, added, unchanged };
}
