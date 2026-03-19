import { useCallback, useMemo, useState } from "react";
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
import { TagInput } from "./composed/TagInput";

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

  const createEntry = useCreateEntry();
  const { data: stats } = useStats();

  const scopeOptions = useMemo(() => buildScopeOptions(stats), [stats]);

  const tagSuggestions = useMemo(() => {
    if (!stats?.entries_by_tag) return [];
    return stats.entries_by_tag.map((t) => t.tag);
  }, [stats]);

  const isValid = title.trim().length > 0 && body.trim().length > 0;

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
        <TagInput
          value={tags}
          onChange={setTags}
          suggestions={tagSuggestions}
        />
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
