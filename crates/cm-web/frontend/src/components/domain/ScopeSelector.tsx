import { useEffect, useMemo, useState } from "react";
import type { Stats } from "@/api/client";
import { useStats } from "@/api/hooks";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ScopeSelector as ScopeSelectorValue } from "@/lib/scope";
import { cn } from "@/lib/utils";

export type { ScopeSelector as ScopeSelectorValue } from "@/lib/scope";
export type SingularScopeSelector = Extract<ScopeSelectorValue, { kind: "path" | "cwd_inferred" }>;

type ScopeMode = "path" | "subtree" | "set" | "all";

const ANY_SCOPE_VALUE = "__cm_any_scope__";
const CWD_INFERRED_VALUE = "__cm_cwd_inferred__";

interface ScopePickerProps {
  value?: SingularScopeSelector;
  onChange: (scope: SingularScopeSelector | undefined) => void;
}

export function ScopePicker({ value, onChange }: ScopePickerProps) {
  const { data: stats } = useStats();
  const options = useMemo(() => buildScopeOptions(stats), [stats]);
  const selectValue =
    value?.kind === "cwd_inferred"
      ? CWD_INFERRED_VALUE
      : value?.kind === "path"
        ? value.path
        : ANY_SCOPE_VALUE;

  return (
    <Select
      value={selectValue}
      onValueChange={(nextValue) => {
        if (nextValue == null || nextValue === ANY_SCOPE_VALUE) {
          onChange(undefined);
          return;
        }
        if (nextValue === CWD_INFERRED_VALUE) {
          onChange({ kind: "cwd_inferred" });
          return;
        }
        onChange({ kind: "path", path: nextValue });
      }}
    >
      <SelectTrigger className="w-full font-mono text-xs">
        <SelectValue placeholder="Any scope" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ANY_SCOPE_VALUE}>Any scope</SelectItem>
        <SelectItem value={CWD_INFERRED_VALUE}>Current directory</SelectItem>
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

interface ScopeSelectorProps {
  value?: ScopeSelectorValue;
  onChange: (scope: ScopeSelectorValue | undefined) => void;
}

export function ScopeSelector({ value, onChange }: ScopeSelectorProps) {
  const { data: stats } = useStats();
  const options = useMemo(() => buildScopeOptions(stats), [stats]);
  const [mode, setMode] = useState<ScopeMode>(modeFromScope(value));
  const path = pathFromScope(value);
  const setPaths = value?.kind === "set" ? value.paths : [];

  useEffect(() => {
    setMode(modeFromScope(value));
  }, [value]);

  const changeMode = (nextMode: ScopeMode) => {
    setMode(nextMode);
    if (nextMode === "all") {
      onChange({ kind: "all" });
      return;
    }
    if (nextMode === "set") {
      onChange(path ? { kind: "set", paths: [path] } : undefined);
      return;
    }
    if (path) {
      onChange(nextMode === "subtree" ? { kind: "subtree", path } : { kind: "path", path });
    } else {
      onChange(undefined);
    }
  };

  const selectPath = (nextPath: string | null) => {
    if (nextPath == null || nextPath === ANY_SCOPE_VALUE) {
      onChange(undefined);
      return;
    }
    onChange(
      mode === "subtree" ? { kind: "subtree", path: nextPath } : { kind: "path", path: nextPath },
    );
  };

  const toggleSetPath = (optionPath: string) => {
    const nextPaths = setPaths.includes(optionPath)
      ? setPaths.filter((path) => path !== optionPath)
      : [...setPaths, optionPath];
    onChange(nextPaths.length === 0 ? { kind: "all" } : { kind: "set", paths: nextPaths });
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Select
        value={mode}
        onValueChange={(nextMode) => {
          if (nextMode) changeMode(nextMode as ScopeMode);
        }}
      >
        <SelectTrigger className="h-7 w-auto min-w-[7.5rem] rounded-md border-border bg-muted px-2 font-mono text-xs text-muted-foreground">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="path">Exact scope</SelectItem>
          <SelectItem value="subtree">Subtree</SelectItem>
          <SelectItem value="set">Set</SelectItem>
          <SelectItem value="all">All scopes</SelectItem>
        </SelectContent>
      </Select>

      {mode !== "set" && (
        <Select
          value={path ?? ANY_SCOPE_VALUE}
          onValueChange={selectPath}
          disabled={mode === "all"}
        >
          <SelectTrigger className="h-7 w-auto min-w-[12rem] rounded-md border-border bg-muted px-2 font-mono text-xs text-muted-foreground">
            <SelectValue placeholder={mode === "all" ? "All scopes" : "Choose scope"} />
          </SelectTrigger>
          <SelectContent className="min-w-[var(--radix-select-trigger-width)] w-auto max-w-[min(500px,90vw)]">
            <SelectItem value={ANY_SCOPE_VALUE}>
              {mode === "all" ? "All scopes" : "Choose scope"}
            </SelectItem>
            {options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
                {option.count != null && (
                  <span className="ml-1 text-muted-foreground/60">({option.count})</span>
                )}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}

      {mode === "set" && (
        <div className="flex max-w-full flex-wrap gap-1">
          {options.map((option) => {
            const selected = setPaths.includes(option.value);
            return (
              <button
                key={option.value}
                type="button"
                onClick={() => toggleSetPath(option.value)}
                className={cn(
                  "rounded-md border px-2 py-1 font-mono text-[11px] transition-colors",
                  selected
                    ? "border-ring bg-accent text-foreground"
                    : "border-border bg-muted text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                {option.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function modeFromScope(scope: ScopeSelectorValue | undefined): ScopeMode {
  if (scope == null) return "path";
  if (scope?.kind === "path" || scope?.kind === "cwd_inferred") return "path";
  if (scope?.kind === "subtree") return "subtree";
  if (scope?.kind === "set") return "set";
  return "all";
}

function pathFromScope(scope: ScopeSelectorValue | undefined): string | undefined {
  if (scope?.kind === "path" || scope?.kind === "subtree") return scope.path;
  if (scope?.kind === "set") return scope.paths[0];
  return undefined;
}

function buildScopeOptions(stats: Stats | undefined) {
  return (
    stats?.scope_tree.map((node) => ({
      value: node.path,
      label: node.path,
      count: node.entry_count,
    })) ?? []
  );
}
