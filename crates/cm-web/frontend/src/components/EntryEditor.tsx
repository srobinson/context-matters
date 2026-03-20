import { useCallback, useMemo, useState } from "react";
import type { EntryDetail, Stats } from "@/api/client";
import type { Confidence } from "@/api/generated/Confidence";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { EntryMeta } from "@/api/generated/EntryMeta";
import { useMergeEntry, useStats, useUpdateEntry } from "@/api/hooks";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { MarkdownContent } from "./composed/MarkdownContent";
import { TagInput } from "./composed/TagInput";
import { DiffView } from "./DiffView";

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

const ALL_CONFIDENCES: (Confidence | "none")[] = ["none", "high", "medium", "low"];

interface EntryEditorProps {
  entry: EntryDetail;
  onCancel: () => void;
  onSaved: () => void;
}

export function EntryEditor({ entry, onCancel, onSaved }: EntryEditorProps) {
  const [title, setTitle] = useState(entry.title);
  const [body, setBody] = useState(entry.body);
  const [kind, setKind] = useState<EntryKind>(entry.kind);
  const [tags, setTags] = useState<string[]>(entry.meta?.tags ?? []);
  const [confidence, setConfidence] = useState<Confidence | "none">(
    entry.meta?.confidence ?? "none",
  );
  const [showPreview, setShowPreview] = useState(false);
  const [pendingScope, setPendingScope] = useState<string | null>(null);
  const [showDiff, setShowDiff] = useState(false);

  const updateEntry = useUpdateEntry();
  const mergeEntry = useMergeEntry();
  const { data: stats } = useStats();

  const scopeOptions = useMemo(() => buildScopeOptions(stats), [stats]);

  const tagSuggestions = useMemo(() => {
    if (!stats?.entries_by_tag) return [];
    return stats.entries_by_tag.map((t) => t.tag);
  }, [stats]);

  const hasChanges =
    title !== entry.title ||
    body !== entry.body ||
    kind !== entry.kind ||
    JSON.stringify(tags) !== JSON.stringify(entry.meta?.tags ?? []) ||
    (confidence === "none" ? null : confidence) !== (entry.meta?.confidence ?? null);

  const isMutating = updateEntry.isPending || mergeEntry.isPending;

  const handleReviewChanges = useCallback(() => {
    setShowDiff(true);
  }, []);

  const handleConfirmSave = useCallback(() => {
    const meta: EntryMeta = {
      ...entry.meta,
      tags: tags.length > 0 ? tags : undefined,
      confidence: confidence === "none" ? null : confidence,
    };

    updateEntry.mutate(
      {
        id: entry.id,
        update: {
          title: title !== entry.title ? title : undefined,
          body: body !== entry.body ? body : undefined,
          kind: kind !== entry.kind ? kind : undefined,
          meta,
        },
      },
      { onSuccess: onSaved },
    );
  }, [entry, title, body, kind, tags, confidence, updateEntry, onSaved]);

  const handleScopeChange = useCallback(
    (newScope: string | null) => {
      if (!newScope) return;
      if (newScope !== entry.scope_path) {
        setPendingScope(newScope);
      }
    },
    [entry.scope_path],
  );

  const handleScopeConfirm = useCallback(() => {
    if (!pendingScope) return;

    const meta: EntryMeta = {
      ...entry.meta,
      tags: tags.length > 0 ? tags : undefined,
      confidence: confidence === "none" ? null : confidence,
    };

    mergeEntry.mutate(
      {
        oldId: entry.id,
        newEntry: {
          scope_path: pendingScope,
          kind,
          title,
          body,
          created_by: entry.created_by,
          meta,
        },
      },
      {
        onSuccess: () => {
          setPendingScope(null);
          onSaved();
        },
      },
    );
  }, [pendingScope, entry, kind, title, body, tags, confidence, mergeEntry, onSaved]);

  return (
    <div className="space-y-4 border-t border-border pt-3" onClick={(e) => e.stopPropagation()}>
      {/* Title */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          title
        </label>
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          className="font-mono text-xs"
        />
      </div>

      {/* Body with preview toggle */}
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            body
          </label>
          <button
            type="button"
            onClick={() => setShowPreview((p) => !p)}
            className="font-mono text-[10px] text-muted-foreground hover:text-foreground"
          >
            {showPreview ? "edit" : "preview"}
          </button>
        </div>
        {showPreview ? (
          <MarkdownContent className="min-h-[120px] rounded-lg border border-input bg-transparent p-3">
            {body}
          </MarkdownContent>
        ) : (
          <Textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            className="min-h-[120px] font-mono text-xs"
          />
        )}
      </div>

      {/* Kind + Confidence row */}
      <div className="flex gap-4">
        <div className="space-y-1">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            kind
          </label>
          <Select value={kind} onValueChange={(v) => setKind(v as EntryKind)}>
            <SelectTrigger className="h-7 w-[140px] font-mono text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {ALL_KINDS.map((k) => (
                <SelectItem key={k} value={k}>
                  {k}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
            confidence
          </label>
          <Select value={confidence} onValueChange={(v) => setConfidence(v as Confidence | "none")}>
            <SelectTrigger className="h-7 w-[120px] font-mono text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {ALL_CONFIDENCES.map((c) => (
                <SelectItem key={c} value={c}>
                  {c}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {/* Scope */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          scope
        </label>
        <Select value={entry.scope_path} onValueChange={handleScopeChange}>
          <SelectTrigger className="h-7 w-full font-mono text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {scopeOptions.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
                {opt.count != null && (
                  <span className="ml-1 text-muted-foreground/60">({opt.count})</span>
                )}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Scope change confirmation */}
      {pendingScope && (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 space-y-2">
          <p className="font-mono text-xs text-foreground">Move entry to a different scope?</p>
          <p className="font-mono text-[11px] text-muted-foreground">
            <span className="line-through">{entry.scope_path}</span>
            {" \u2192 "}
            <span className="font-medium text-foreground">{pendingScope}</span>
          </p>
          <p className="font-mono text-[10px] text-muted-foreground/80">
            This creates a new entry at the target scope and supersedes the original. The operation
            cannot be undone.
          </p>
          <div className="flex items-center gap-2 pt-1">
            <button
              type="button"
              onClick={handleScopeConfirm}
              disabled={mergeEntry.isPending}
              className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-1 font-mono text-xs text-foreground transition-colors hover:bg-amber-500/20 disabled:opacity-50"
            >
              {mergeEntry.isPending ? "moving..." : "confirm move"}
            </button>
            <button
              type="button"
              onClick={() => setPendingScope(null)}
              disabled={mergeEntry.isPending}
              className="font-mono text-[11px] text-muted-foreground hover:text-foreground"
            >
              cancel
            </button>
            {mergeEntry.isError && (
              <span className="font-mono text-xs text-destructive">{mergeEntry.error.message}</span>
            )}
          </div>
        </div>
      )}

      {/* Tags */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          tags
        </label>
        <TagInput value={tags} onChange={setTags} suggestions={tagSuggestions} maxSuggestions={8} />
      </div>

      {/* Created by (read-only) */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          created by
        </label>
        <p className="font-mono text-xs text-muted-foreground">{entry.created_by}</p>
      </div>

      {/* Actions / Diff view */}
      {showDiff ? (
        <DiffView
          entry={entry}
          edited={{ title, body, kind, tags, confidence }}
          onConfirm={handleConfirmSave}
          onBack={() => setShowDiff(false)}
          isPending={updateEntry.isPending}
          error={updateEntry.isError ? updateEntry.error.message : null}
        />
      ) : (
        <div className="flex items-center gap-2 pt-1">
          <button
            type="button"
            onClick={handleReviewChanges}
            disabled={!hasChanges || isMutating}
            className="rounded-md border border-border bg-foreground px-3 py-1.5 font-mono text-xs text-background transition-colors hover:bg-foreground/90 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            save
          </button>
          <button
            type="button"
            onClick={onCancel}
            disabled={isMutating}
            className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
          >
            cancel
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
