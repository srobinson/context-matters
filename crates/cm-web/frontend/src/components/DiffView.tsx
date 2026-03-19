import { useMemo } from "react";
import type { Confidence } from "@/api/generated/Confidence";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { EntryDetail } from "@/api/client";
import {
  DiffView as ComposedDiffView,
  type FieldChange,
} from "./composed/DiffView";

export interface DiffFields {
  title: string;
  body: string;
  kind: EntryKind;
  tags: string[];
  confidence: Confidence | "none";
}

interface DiffViewProps {
  entry: EntryDetail;
  edited: DiffFields;
  onConfirm: () => void;
  onBack: () => void;
  isPending: boolean;
  error?: string | null;
}

export function DiffView({
  entry,
  edited,
  onConfirm,
  onBack,
  isPending,
  error,
}: DiffViewProps) {
  const changes = useMemo(
    () => computeChanges(entry, edited),
    [entry, edited],
  );

  return (
    <ComposedDiffView
      changes={changes}
      onConfirm={onConfirm}
      onBack={onBack}
      isPending={isPending}
      error={error}
    />
  );
}

function computeChanges(
  entry: EntryDetail,
  edited: DiffFields,
): FieldChange[] {
  const changes: FieldChange[] = [];

  if (edited.title !== entry.title) {
    changes.push({
      label: "title",
      original: entry.title,
      current: edited.title,
    });
  }

  if (edited.body !== entry.body) {
    changes.push({
      label: "body",
      original: entry.body,
      current: edited.body,
    });
  }

  if (edited.kind !== entry.kind) {
    changes.push({
      label: "kind",
      original: entry.kind,
      current: edited.kind,
    });
  }

  const originalTags = (entry.meta?.tags ?? []).join(", ");
  const currentTags = edited.tags.join(", ");
  if (originalTags !== currentTags) {
    changes.push({
      label: "tags",
      original: originalTags || "(none)",
      current: currentTags || "(none)",
    });
  }

  const originalConf = entry.meta?.confidence ?? "none";
  if (edited.confidence !== originalConf) {
    changes.push({
      label: "confidence",
      original: String(originalConf),
      current: String(edited.confidence),
    });
  }

  return changes;
}
