import { useMemo } from "react";
import type { Stats } from "@/api/client";
import type { EntryKind } from "@/api/generated/EntryKind";
import { useStats } from "@/api/hooks";
import { TagInput } from "@/components/composed/TagInput";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

const ALL_KINDS: EntryKind[] = [
  "fact",
  "decision",
  "preference",
  "lesson",
  "reference",
  "feedback",
  "pattern",
  "observation",
];

interface RecallBarProps {
  scope?: string;
  kinds: EntryKind[];
  tags: string[];
  limit: number;
  maxTokens?: number;
  onScopeChange: (scope?: string) => void;
  onKindsChange: (kinds: EntryKind[]) => void;
  onTagsChange: (tags: string[]) => void;
  onLimitChange: (limit: number) => void;
  onMaxTokensChange: (maxTokens?: number) => void;
  onClear: () => void;
}

export function RecallBar({
  scope,
  kinds,
  tags,
  limit,
  maxTokens,
  onScopeChange,
  onKindsChange,
  onTagsChange,
  onLimitChange,
  onMaxTokensChange,
  onClear,
}: RecallBarProps) {
  const { data: stats } = useStats();

  const scopeOptions = useMemo(() => buildScopeOptions(stats), [stats]);
  const tagSuggestions = useMemo(
    () =>
      stats?.entries_by_tag.map((entry) => ({
        value: entry.tag,
        label: `${entry.tag} (${entry.count})`,
      })) ?? [],
    [stats],
  );

  return (
    <section className="space-y-3 rounded-lg border border-border bg-card/60 p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="space-y-1">
          <p className="font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/60">
            recall
          </p>
          <p className="text-sm text-muted-foreground">
            Matches `cx_recall`: optional query, single scope, multi kind, multi tag.
          </p>
        </div>
        <button
          type="button"
          onClick={onClear}
          className="font-mono text-[10px] text-muted-foreground/60 hover:text-foreground"
        >
          clear recall
        </button>
      </div>

      <div className="grid gap-3 xl:grid-cols-[minmax(0,18rem)_minmax(0,1fr)_8rem_8rem]">
        <div className="space-y-1">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            scope
          </label>
          <Select value={scope} onValueChange={(value) => onScopeChange(value ?? undefined)}>
            <SelectTrigger className="w-full font-mono text-xs">
              <SelectValue placeholder="Any scope" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="">Any scope</SelectItem>
              {scopeOptions.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            tags
          </label>
          <TagInput
            value={tags}
            onChange={onTagsChange}
            suggestions={tagSuggestions}
            placeholder="Any tags..."
            maxSuggestions={undefined}
          />
        </div>

        <div className="space-y-1">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            limit
          </label>
          <Input
            type="number"
            min={1}
            max={200}
            value={String(limit)}
            onChange={(event) => {
              const next = Number(event.target.value);
              if (Number.isFinite(next)) {
                onLimitChange(next);
              }
            }}
            className="font-mono text-xs"
          />
        </div>

        <div className="space-y-1">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            max tokens
          </label>
          <Input
            type="number"
            min={1}
            value={maxTokens == null ? "" : String(maxTokens)}
            onChange={(event) => {
              const raw = event.target.value.trim();
              if (!raw) {
                onMaxTokensChange(undefined);
                return;
              }
              const next = Number(raw);
              if (Number.isFinite(next)) {
                onMaxTokensChange(next);
              }
            }}
            placeholder="none"
            className="font-mono text-xs"
          />
        </div>
      </div>

      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          kinds
        </label>
        <div className="flex flex-wrap gap-1.5">
          {ALL_KINDS.map((kind) => {
            const selected = kinds.includes(kind);
            return (
              <button
                key={kind}
                type="button"
                onClick={() =>
                  onKindsChange(
                    selected ? kinds.filter((value) => value !== kind) : [...kinds, kind],
                  )
                }
                className={cn(
                  "rounded-md border px-2 py-1 font-mono text-[11px] transition-colors",
                  selected
                    ? "border-ring bg-accent text-foreground"
                    : "border-border bg-muted text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                {kind}
              </button>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function buildScopeOptions(stats: Stats | undefined) {
  return (
    stats?.scope_tree.map((node) => ({
      value: node.path,
      label: node.path,
    })) ?? []
  );
}
