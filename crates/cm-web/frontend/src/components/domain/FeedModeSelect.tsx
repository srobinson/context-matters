export type FeedMode = "curate" | "recall" | "browse";

const modes: { value: FeedMode; label: string }[] = [
  { value: "curate", label: "Curate" },
  { value: "recall", label: "Recall" },
  { value: "browse", label: "Browse" },
];

export function FeedModeSelect({
  value,
  onChange,
}: {
  value: FeedMode;
  onChange: (mode: FeedMode) => void;
}) {
  return (
    <div className="inline-flex items-center rounded-md border border-border bg-muted p-0.5">
      {modes.map((m) => (
        <button
          key={m.value}
          type="button"
          onClick={() => onChange(m.value)}
          className={`rounded px-2.5 py-0.5 font-mono text-xs transition-colors ${
            value === m.value
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
