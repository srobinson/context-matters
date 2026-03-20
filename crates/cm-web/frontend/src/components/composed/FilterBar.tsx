import { X } from "lucide-react";
import { useCallback } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

// --- Public types ---

export interface FacetOption {
  value: string;
  label: string;
  count?: number;
}

export interface FacetDefinition {
  key: string;
  placeholder: string;
  options: FacetOption[];
}

export interface ToggleDefinition {
  key: string;
  label: string;
}

export interface FilterBarProps {
  /** Select-based facet filters */
  facets: FacetDefinition[];
  /** Checkbox toggles */
  toggles?: ToggleDefinition[];
  /** Current filter values by key */
  values: Record<string, string | boolean | undefined>;
  /** Called when any filter value changes */
  onChange: (key: string, value: string | boolean | undefined) => void;
  /** Called to clear all filters */
  onClearAll?: () => void;
  /** Format chip label from key+value. Default: "key:value" */
  chipLabel?: (key: string, value: string | boolean) => string;
}

// --- Internal components ---

function ActiveChip({ label, onRemove }: { label: string; onRemove: () => void }) {
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

function FacetSelect({
  placeholder,
  value,
  onChange,
  options,
}: {
  placeholder: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  options: FacetOption[];
}) {
  return (
    <Select value={value} onValueChange={(v) => onChange(v == null || v === "" ? undefined : v)}>
      <SelectTrigger className="h-7 w-auto min-w-[100px] gap-1 rounded-md border-border bg-muted px-2 font-mono text-xs text-muted-foreground">
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent className="min-w-[var(--radix-select-trigger-width)] w-auto max-w-[min(500px,90vw)]">
        <SelectItem value="">{`Any ${placeholder.toLowerCase()}`}</SelectItem>
        {options.map((opt) => (
          <SelectItem key={opt.value} value={opt.value}>
            {opt.label}
            {opt.count != null && (
              <span className="ml-1 text-muted-foreground/60">({opt.count})</span>
            )}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

// --- Main component ---

export function FilterBar({
  facets,
  toggles,
  values,
  onChange,
  onClearAll,
  chipLabel,
}: FilterBarProps) {
  const defaultChipLabel = useCallback((key: string, value: string | boolean) => {
    if (typeof value === "boolean") return `show:${key}`;
    return `${key}:${value}`;
  }, []);

  const formatChip = chipLabel ?? defaultChipLabel;

  const activeEntries = Object.entries(values).filter(([, v]) => v !== undefined && v !== false);
  const hasActiveFilters = activeEntries.length > 0;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        {facets.map((facet) => (
          <FacetSelect
            key={facet.key}
            placeholder={facet.placeholder}
            value={
              typeof values[facet.key] === "string" ? (values[facet.key] as string) : undefined
            }
            onChange={(v) => onChange(facet.key, v)}
            options={facet.options}
          />
        ))}

        {toggles?.map((toggle) => (
          <label
            key={toggle.key}
            className="inline-flex cursor-pointer items-center gap-1.5 font-mono text-xs text-muted-foreground"
          >
            <input
              type="checkbox"
              checked={!!values[toggle.key]}
              onChange={(e) => onChange(toggle.key, e.target.checked || undefined)}
              className={cn("h-3.5 w-3.5 rounded border-border accent-foreground")}
            />
            {toggle.label}
          </label>
        ))}
      </div>

      {hasActiveFilters && (
        <div className="flex flex-wrap items-center gap-1.5">
          {activeEntries.map(([key, value]) => (
            <ActiveChip
              key={key}
              label={formatChip(key, value as string | boolean)}
              onRemove={() => onChange(key, undefined)}
            />
          ))}
          {onClearAll && (
            <button
              type="button"
              onClick={onClearAll}
              className="font-mono text-[10px] text-muted-foreground/60 hover:text-foreground"
            >
              clear all
            </button>
          )}
        </div>
      )}
    </div>
  );
}
