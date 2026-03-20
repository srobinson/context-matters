import { useCallback, useEffect, useRef, useState } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

export interface InlineEditProps {
  /** Current display value */
  value: string;
  /** Called with new value on save */
  onSave: (value: string) => void;
  /** Called when edit is cancelled */
  onCancel?: () => void;
  /** "text" for single-line, "textarea" for multi-line */
  mode?: "text" | "textarea";
  /** Validation function. Return error string or undefined if valid. */
  validate?: (value: string) => string | undefined;
  /** Placeholder text for the input */
  placeholder?: string;
  /** Additional className for the display element */
  className?: string;
}

export function InlineEdit({
  value,
  onSave,
  onCancel,
  mode = "text",
  validate,
  placeholder,
  className,
}: InlineEditProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | undefined>();
  const inputRef = useRef<HTMLInputElement | HTMLTextAreaElement>(null);

  const isDirty = draft !== value;

  useEffect(() => {
    if (isEditing) {
      inputRef.current?.focus();
    }
  }, [isEditing]);

  const handleEnterEdit = useCallback(() => {
    setDraft(value);
    setError(undefined);
    setIsEditing(true);
  }, [value]);

  const handleCancel = useCallback(() => {
    setDraft(value);
    setError(undefined);
    setIsEditing(false);
    onCancel?.();
  }, [value, onCancel]);

  const handleSave = useCallback(() => {
    if (!isDirty) {
      setIsEditing(false);
      return;
    }
    const err = validate?.(draft);
    if (err) {
      setError(err);
      return;
    }
    onSave(draft);
    setIsEditing(false);
    setError(undefined);
  }, [draft, isDirty, validate, onSave]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        handleCancel();
      } else if (e.key === "Enter" && mode === "text") {
        e.preventDefault();
        handleSave();
      } else if (e.key === "Enter" && e.metaKey && mode === "textarea") {
        e.preventDefault();
        handleSave();
      }
    },
    [handleCancel, handleSave, mode],
  );

  if (!isEditing) {
    return (
      <button
        type="button"
        onClick={handleEnterEdit}
        className={`text-left font-mono text-xs text-foreground hover:bg-accent/30 rounded px-1 -mx-1 py-0.5 transition-colors ${className ?? ""}`}
      >
        {value || <span className="text-muted-foreground/40">{placeholder}</span>}
      </button>
    );
  }

  const InputComponent = mode === "textarea" ? Textarea : Input;

  return (
    <div className="space-y-1">
      <InputComponent
        ref={inputRef as never}
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          if (error) setError(undefined);
        }}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        className={`font-mono text-xs ${mode === "textarea" ? "min-h-[80px]" : ""}`}
      />
      {error && <p className="font-mono text-[10px] text-destructive">{error}</p>}
      <div className="flex items-center gap-1.5">
        <button
          type="button"
          onClick={handleSave}
          disabled={!isDirty}
          className="rounded-md bg-foreground px-2 py-0.5 font-mono text-[10px] text-background hover:bg-foreground/90 disabled:opacity-50"
        >
          save
        </button>
        <button
          type="button"
          onClick={handleCancel}
          className="font-mono text-[10px] text-muted-foreground hover:text-foreground"
        >
          cancel
        </button>
      </div>
    </div>
  );
}
