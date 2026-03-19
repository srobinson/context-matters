import { useCallback, useMemo } from "react";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { Stats } from "@/api/client";
import { useStats } from "@/api/hooks";
import {
  FilterBar as ComposedFilterBar,
  type FacetDefinition,
} from "./composed/FilterBar";

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

const CHIP_LABELS: Record<string, string> = {
  scope_path: "scope",
  kind: "kind",
  created_by: "by",
  tag: "tag",
  show_forgotten: "show:forgotten",
};

export function FilterBar({
  filters,
  onChange,
}: {
  filters: FilterState;
  onChange: (update: Partial<FilterState>) => void;
}) {
  const { data: stats } = useStats();

  const facets = useMemo(() => buildFacets(stats), [stats]);

  const values: Record<string, string | boolean | undefined> = {
    scope_path: filters.scope_path,
    kind: filters.kind,
    created_by: filters.created_by,
    tag: filters.tag,
    show_forgotten: filters.show_forgotten,
  };

  const handleChange = useCallback(
    (key: string, value: string | boolean | undefined) => {
      onChange({ [key]: value });
    },
    [onChange],
  );

  const handleClearAll = useCallback(() => {
    onChange({
      scope_path: undefined,
      kind: undefined,
      tag: undefined,
      created_by: undefined,
      show_forgotten: undefined,
    });
  }, [onChange]);

  const chipLabel = useCallback(
    (key: string, value: string | boolean) => {
      if (typeof value === "boolean") return CHIP_LABELS[key] ?? key;
      return `${CHIP_LABELS[key] ?? key}:${value}`;
    },
    [],
  );

  return (
    <ComposedFilterBar
      facets={facets}
      toggles={[{ key: "show_forgotten", label: "forgotten" }]}
      values={values}
      onChange={handleChange}
      onClearAll={handleClearAll}
      chipLabel={chipLabel}
    />
  );
}

function buildFacets(stats: Stats | undefined): FacetDefinition[] {
  const scopeOptions = stats?.scope_tree
    ? stats.scope_tree.map((node) => ({
        value: node.path,
        label: node.path,
        count: node.entry_count,
      }))
    : [];

  const kindOptions = ALL_KINDS.map((k) => ({
    value: k,
    label: k,
    count: stats?.entries_by_kind[k] ?? undefined,
  }));

  const agentOptions = stats?.active_agents
    ? stats.active_agents.map((agent) => ({
        value: agent.created_by,
        label: agent.created_by.replace(/^agent:/, ""),
        count: agent.count,
      }))
    : [];

  const tagOptions = stats?.entries_by_tag
    ? stats.entries_by_tag.map((t) => ({
        value: t.tag,
        label: t.tag,
        count: t.count,
      }))
    : [];

  return [
    { key: "scope_path", placeholder: "Scope", options: scopeOptions },
    { key: "kind", placeholder: "Kind", options: kindOptions },
    { key: "created_by", placeholder: "Agent", options: agentOptions },
    { key: "tag", placeholder: "Tag", options: tagOptions },
  ];
}
