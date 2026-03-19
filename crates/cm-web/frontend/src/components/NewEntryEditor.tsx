import { useCallback, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import type { Confidence } from "@/api/generated/Confidence";
import type { EntryKind } from "@/api/generated/EntryKind";
import type { EntryMeta } from "@/api/generated/EntryMeta";
import type { Stats } from "@/api/client";
import { useCreateEntry, useStats } from "@/api/hooks";
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

interface NewEntryEditorProps {
  onCancel: () => void;
  onCreated: () => void;
}

export function NewEntryEditor({ onCancel, onCreated }: NewEntryEditorProps) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [kind, setKind] = useState<EntryKind>("fact");
  const [scopePath, setScopePath] = useState("global");
  const [tags, setTags] = useState<string[]>([]);
  const [confidence, setConfidence] = useState<Confidence | "none">("none");
  const [tagInput, setTagInput] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);
  const tagInputRef = useRef<HTMLInputElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  const createEntry = useCreateEntry();
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

  const isValid = title.trim().length > 0 && body.trim().length > 0;

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

  const handleCreate = useCallback(() => {
    if (!isValid) return;

    const meta: EntryMeta = {
      tags: tags.length > 0 ? tags : undefined,
      confidence: confidence === "none" ? null : confidence,
    };

    createEntry.mutate(
      {
        scope_path: scopePath,
        kind,
        title: title.trim(),
        body: body.trim(),
        created_by: "user:web",
        meta,
      },
      {
        onSuccess: () => {
          toast.success("Entry created");
          onCreated();
        },
      },
    );
  }, [isValid, scopePath, kind, title, body, tags, confidence, createEntry, onCreated]);

  return (
    <div
      className="rounded-lg border border-border bg-card p-4 space-y-4"
      onClick={(e) => e.stopPropagation()}
    >
      <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
        new entry
      </p>

      {/* Title */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          title
        </label>
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Short summary..."
          className="font-mono text-xs"
          autoFocus
        />
      </div>

      {/* Body */}
      <div className="space-y-1">
        <label className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          body
        </label>
        <Textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="Markdown content..."
          className="min-h-[120px] font-mono text-xs"
        />
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
        <Select value={scopePath} onValueChange={setScopePath}>
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

      {/* Actions */}
      <div className="flex items-center gap-2 pt-1">
        <button
          type="button"
          onClick={handleCreate}
          disabled={!isValid || createEntry.isPending}
          className="rounded-md border border-border bg-foreground px-3 py-1.5 font-mono text-xs text-background transition-colors hover:bg-foreground/90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {createEntry.isPending ? "creating..." : "create"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          disabled={createEntry.isPending}
          className="rounded-md border border-border bg-muted px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
        >
          cancel
        </button>
        {createEntry.isError && (
          <span className="font-mono text-xs text-destructive">
            {createEntry.error.message}
          </span>
        )}
      </div>
    </div>
  );
}

function buildScopeOptions(stats: Stats | undefined) {
  if (!stats?.scope_tree) return [{ value: "global", label: "global", count: undefined as number | undefined }];
  return stats.scope_tree.map((node) => ({
    value: node.path,
    label: node.path,
    count: node.entry_count as number | undefined,
  }));
}
