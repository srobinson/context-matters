import { X } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

export interface TagInputProps {
  /** Current tags */
  value: string[];
  /** Called with updated tag array */
  onChange: (tags: string[]) => void;
  /** Available suggestions (excluding current tags) */
  suggestions?: Array<string | { value: string; label: string }>;
  /** Input placeholder when no tags present */
  placeholder?: string;
  /** Max visible suggestions in dropdown. Omit for no cap. */
  maxSuggestions?: number;
}

export function TagInput({
  value,
  onChange,
  suggestions = [],
  placeholder = "Add tags...",
  maxSuggestions,
}: TagInputProps) {
  const [input, setInput] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  const normalizedSuggestions = useMemo(
    () =>
      suggestions.map((suggestion) =>
        typeof suggestion === "string" ? { value: suggestion, label: suggestion } : suggestion,
      ),
    [suggestions],
  );

  const availableSuggestions = useMemo(
    () => normalizedSuggestions.filter((s) => !value.includes(s.value)),
    [normalizedSuggestions, value],
  );

  const filteredSuggestions = useMemo(() => {
    const limit = (items: Array<{ value: string; label: string }>) =>
      maxSuggestions == null ? items : items.slice(0, maxSuggestions);

    if (!input.trim()) return limit(availableSuggestions);
    const query = input.toLowerCase();
    return limit(
      availableSuggestions.filter(
        (s) => s.value.toLowerCase().includes(query) || s.label.toLowerCase().includes(query),
      ),
    );
  }, [input, availableSuggestions, maxSuggestions]);

  const handleAdd = useCallback(
    (tag: string) => {
      const trimmed = tag.trim().toLowerCase();
      if (trimmed && !value.includes(trimmed)) {
        onChange([...value, trimmed]);
      }
      setInput("");
      setShowSuggestions(false);
      inputRef.current?.focus();
    },
    [value, onChange],
  );

  const handleRemove = useCallback(
    (tag: string) => {
      onChange(value.filter((t) => t !== tag));
    },
    [value, onChange],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" || e.key === ",") {
        e.preventDefault();
        if (input.trim()) handleAdd(input);
      } else if (e.key === "Backspace" && !input && value.length > 0) {
        onChange(value.slice(0, -1));
      } else if (e.key === "Escape") {
        setShowSuggestions(false);
      }
    },
    [input, value, onChange, handleAdd],
  );

  return (
    <div className="flex flex-wrap items-center gap-1 rounded-lg border border-input bg-transparent p-1.5 focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 dark:bg-input/30">
      {value.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center gap-0.5 rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
        >
          {tag}
          <button
            type="button"
            onClick={() => handleRemove(tag)}
            className="ml-0.5 rounded-sm p-0.5 hover:bg-accent hover:text-foreground"
          >
            <X className="h-2.5 w-2.5" />
          </button>
        </span>
      ))}
      <div className="relative flex-1">
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => {
            setInput(e.target.value);
            setShowSuggestions(true);
          }}
          onFocus={() => setShowSuggestions(true)}
          onBlur={() => {
            setTimeout(() => setShowSuggestions(false), 150);
          }}
          onKeyDown={handleKeyDown}
          placeholder={value.length === 0 ? placeholder : ""}
          className="h-6 w-full min-w-[60px] border-0 bg-transparent p-0 px-1 font-mono text-[11px] outline-none placeholder:text-muted-foreground/40"
        />
        {showSuggestions && filteredSuggestions.length > 0 && (
          <div
            ref={suggestionsRef}
            className="absolute left-0 top-full z-50 mt-1 max-h-[160px] w-[200px] overflow-y-auto rounded-overlay border border-border bg-popover p-1 shadow-overlay"
          >
            {filteredSuggestions.map((suggestion) => (
              <button
                key={suggestion.value}
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleAdd(suggestion.value)}
                className="block w-full rounded-sm px-2 py-1 text-left font-mono text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground"
              >
                {suggestion.label}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
