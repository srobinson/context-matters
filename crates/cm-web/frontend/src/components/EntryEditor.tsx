import { useCallback, useMemo, useRef, useState } from "react";
import Markdown from "react-markdown";
import type { Confidence } from "@/api/generated/Confidence";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { EntryMeta } from "@/api/generated/EntryMeta";
import type { Stats, EntryDetail } from "@/api/client";
import { useUpdateEntry, useMergeEntry, useStats } from "@/api/hooks";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { X } from "lucide-react";
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

const ALL_CONFIDENCES: (Confidence | "none")[] = [
  "none",
  "high",
  "medium",
  "low",
];

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
  const [tagInput, setTagInput] = useState("");
  const [showPreview, setShowPreview] = useState(false);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [pendingScope, setPendingScope] = useState<string | null>(null);
  const [showDiff, setShowDiff] = useState(false);
  const tagInputRef = useRef<HTMLInputElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  const updateEntry = useUpdateEntry();
  const mergeEntry = useMergeEntry();
  const { data: stats } = useStats();

  const scopeOptions = useMemo(() => buildScopeOptions(stats), [stats]);

  const existingTags = useMemo(() => {
    if (!stats?.entries_by_tag) return [];
    return stats.entries_by_tag
      .map((t) => t.tag)
      .filter((t) => !tags.includes(t));
  }, [stats, tags]);

  const filteredSuggestions = useMemo(() => {
    if (!tagInput.trim()) return existingTags.slice(0, 8);
    const query = tagInput.toLowerCase();
    return existingTags
      .filter((t) => t.toLowerCase().includes(query))
      .slice(0, 8);
  }, [tagInput, existingTags]);

  const hasChanges =
    title !== entry.title ||
    body !== entry.body ||
    kind !== entry.kind ||
    JSON.stringify(tags) !== JSON.stringify(entry.meta?.tags ?? []) ||
    (confidence === "none" ? null : confidence) !==
      (entry.meta?.confidence ?? null);

  const isMutating = updateEntry.isPending || mergeEntry.isPending;

  const handleAddTag = useCallback(
    (tag: string) => {
      const trimmed = tag.trim().toLowerCase();
      if (trimmed && !tags.includes(trimmed)) {
        setTags((prev) => [...prev, trimmed]);
      }
      setTagInput("");
      setShowSuggestions(false);
      tagInputRef.current?.focus();
    },
    [tags],
  );

  const handleRemoveTag = useCallback((tag: string) => {
    setTags((prev) => prev.filter((t) => t !== tag));
  }, []);

  const handleTagKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" || e.key === ",") {
        e.preventDefault();
        if (tagInput.trim()) {
          handleAddTag(tagInput);
        }
      } else if (e.key === "Backspace" && !tagInput && tags.length > 0) {
        setTags((prev) => prev.slice(0, -1));
      } else if (e.key === "Escape") {
        setShowSuggestions(false);
      }
    },
    [tagInput, tags.length, handleAddTag],
  );

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
    (newScope: string) => {
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
  }, [
    pendingScope,
    entry,
    kind,
    title,
    body,
    tags,
    confidence,
    mergeEntry,
    onSaved,
  ]);

  return (
    <div
      className="space-y-4 border-t border-border pt-3"
      onClick={(e) => e.stopPropagation()}
    >
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
          <div className="min-h-[120px] rounded-lg border border-input bg-transparent p-3 prose prose-sm prose-neutral dark:prose-invert max-w-none font-mono text-xs leading-relaxed [&_pre]:bg-muted [&_pre]:p-3 [&_pre]:rounded-md [&_code]:text-[11px] [&_h1]:text-sm [&_h2]:text-sm [&_h3]:text-xs [&_p]:text-xs [&_li]:text-xs">
            <Markdown>{body}</Markdown>
          </div>
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
          <Select
            value={confidence}
            onValueChange={(v) => setConfidence(v as Confidence | "none")}
          >
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
        <Select
          value={entry.scope_path}
          onValueChange={handleScopeChange}
        >
          <SelectTrigger className="h-7 w-full font-mono text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {scopeOptions.map((opt) => (
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
      </div>

      {/* Scope change confirmation */}
      {pendingScope && (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 space-y-2">
          <p className="font-mono text-xs text-foreground">
            Move entry to a different scope?
          </p>
          <p className="font-mono text-[11px] text-muted-foreground">
            <span className="line-through">{entry.scope_path}</span>
            {" \u2192 "}
            <span className="font-medium text-foreground">{pendingScope}</span>
          </p>
          <p className="font-mono text-[10px] text-muted-foreground/80">
            This creates a new entry at the target scope and supersedes the
            original. The operation cannot be undone.
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
              <span className="font-mono text-xs text-destructive">
                {mergeEntry.error.message}
              </span>
            )}
          </div>
        </div>
      )}

      {/* Tags */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          tags
        </label>
        <div className="flex flex-wrap items-center gap-1 rounded-lg border border-input bg-transparent p-1.5 focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 dark:bg-input/30">
          {tags.map((tag) => (
            <span
              key={tag}
              className="inline-flex items-center gap-0.5 rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
            >
              {tag}
              <button
                type="button"
                onClick={() => handleRemoveTag(tag)}
                className="ml-0.5 rounded-sm p-0.5 hover:bg-accent hover:text-foreground"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
          <div className="relative flex-1">
            <input
              ref={tagInputRef}
              type="text"
              value={tagInput}
              onChange={(e) => {
                setTagInput(e.target.value);
                setShowSuggestions(true);
              }}
              onFocus={() => setShowSuggestions(true)}
              onBlur={() => {
                setTimeout(() => setShowSuggestions(false), 150);
              }}
              onKeyDown={handleTagKeyDown}
              placeholder={tags.length === 0 ? "Add tags..." : ""}
              className="h-6 w-full min-w-[60px] border-0 bg-transparent p-0 px-1 font-mono text-[11px] outline-none placeholder:text-muted-foreground/40"
            />
            {showSuggestions && filteredSuggestions.length > 0 && (
              <div
                ref={suggestionsRef}
                className="absolute left-0 top-full z-50 mt-1 max-h-[160px] w-[200px] overflow-y-auto rounded-md border border-border bg-popover p-1 shadow-md"
              >
                {filteredSuggestions.map((suggestion) => (
                  <button
                    key={suggestion}
                    type="button"
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => handleAddTag(suggestion)}
                    className="block w-full rounded-sm px-2 py-1 text-left font-mono text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground"
                  >
                    {suggestion}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Created by (read-only) */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          created by
        </label>
        <p className="font-mono text-xs text-muted-foreground">
          {entry.created_by}
        </p>
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
