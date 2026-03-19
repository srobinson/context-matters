import { useCallback } from "react";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { Stats } from "@/api/client";
import { useStats } from "@/api/hooks";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

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

export type FilterState = {
  scope_path?: string;
  kind?: EntryKind;
  tag?: string;
  created_by?: string;
  show_forgotten?: boolean;
};

function ActiveChip({
  label,
  onRemove,
}: {
  label: string;
  onRemove: () => void;
}) {
  return (
    <span className="inline-flex items-center gap-1 rounded-md border border-border bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground">
      {label}
      <button
        type="button"
        onClick={onRemove}
        className="ml-0.5 rounded-sm p-0.5 hover:bg-accent hover:text-foreground"
      >
        <X className="h-3 w-3" />
      </button>
    </span>
  );
}

function FilterSelect({
  placeholder,
  value,
  onChange,
  options,
}: {
  placeholder: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  options: { value: string; label: string; count?: number }[];
}) {
  return (
    <Select
      value={value ?? "__all__"}
      onValueChange={(v) => onChange(v === "__all__" ? undefined : v)}
    >
      <SelectTrigger className="h-7 w-auto min-w-[100px] gap-1 rounded-md border-border bg-muted px-2 font-mono text-xs text-muted-foreground">
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="__all__">All</SelectItem>
        {options.map((opt) => (
          <SelectItem key={opt.value} value={opt.value}>
            {opt.label}
            {opt.count != null && (
              <span className="ml-1 text-muted-foreground/60">
                ({opt.count})
              </span>
            )}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function FilterBar({
  filters,
  onChange,
}: {
  filters: FilterState;
  onChange: (update: Partial<FilterState>) => void;
}) {
  const { data: stats } = useStats();

  const handleClearAll = useCallback(() => {
    onChange({
      scope_path: undefined,
      kind: undefined,
      tag: undefined,
      created_by: undefined,
      show_forgotten: undefined,
    });
  }, [onChange]);

  const kindOptions = ALL_KINDS.map((k) => ({
    value: k,
    label: k,
    count: stats?.entries_by_kind[k] ?? undefined,
  }));

  const scopeOptions = buildScopeOptions(stats);
  const agentOptions = buildAgentOptions(stats);
  const tagOptions = buildTagOptions(stats);

  const hasActiveFilters =
    filters.scope_path ||
    filters.kind ||
    filters.tag ||
    filters.created_by ||
    filters.show_forgotten;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <FilterSelect
          placeholder="Scope"
          value={filters.scope_path}
          onChange={(v) => onChange({ scope_path: v })}
          options={scopeOptions}
        />
        <FilterSelect
          placeholder="Kind"
          value={filters.kind}
          onChange={(v) => onChange({ kind: v as EntryKind | undefined })}
          options={kindOptions}
        />
        <FilterSelect
          placeholder="Agent"
          value={filters.created_by}
          onChange={(v) => onChange({ created_by: v })}
          options={agentOptions}
        />
        <FilterSelect
          placeholder="Tag"
          value={filters.tag}
          onChange={(v) => onChange({ tag: v })}
          options={tagOptions}
        />

        <label className="inline-flex cursor-pointer items-center gap-1.5 font-mono text-xs text-muted-foreground">
          <input
            type="checkbox"
            checked={filters.show_forgotten ?? false}
            onChange={(e) =>
              onChange({
                show_forgotten: e.target.checked || undefined,
              })
            }
            className={cn(
              "h-3.5 w-3.5 rounded border-border accent-foreground",
            )}
          />
          forgotten
        </label>
      </div>

      {hasActiveFilters && (
        <div className="flex flex-wrap items-center gap-1.5">
          {filters.scope_path && (
            <ActiveChip
              label={`scope:${filters.scope_path}`}
              onRemove={() => onChange({ scope_path: undefined })}
            />
          )}
          {filters.kind && (
            <ActiveChip
              label={`kind:${filters.kind}`}
              onRemove={() => onChange({ kind: undefined })}
            />
          )}
          {filters.created_by && (
            <ActiveChip
              label={`by:${filters.created_by}`}
              onRemove={() => onChange({ created_by: undefined })}
            />
          )}
          {filters.tag && (
            <ActiveChip
              label={`tag:${filters.tag}`}
              onRemove={() => onChange({ tag: undefined })}
            />
          )}
          {filters.show_forgotten && (
            <ActiveChip
              label="show:forgotten"
              onRemove={() => onChange({ show_forgotten: undefined })}
            />
          )}
          <button
            type="button"
            onClick={handleClearAll}
            className="font-mono text-[10px] text-muted-foreground/60 hover:text-foreground"
          >
            clear all
          </button>
        </div>
      )}
    </div>
  );
}

function buildScopeOptions(stats: Stats | undefined) {
  if (!stats?.scope_tree) return [];
  return stats.scope_tree.map((node) => ({
    value: node.path,
    label: node.path,
    count: node.entry_count,
  }));
}

function buildAgentOptions(stats: Stats | undefined) {
  if (!stats?.active_agents) return [];
  return stats.active_agents.map((agent) => ({
    value: agent.created_by,
    label: agent.created_by.replace(/^agent:/, ""),
    count: agent.count,
  }));
}

function buildTagOptions(stats: Stats | undefined) {
  if (!stats?.entries_by_tag) return [];
  return stats.entries_by_tag.map((t) => ({
    value: t.tag,
    label: t.tag,
    count: t.count,
  }));
}
