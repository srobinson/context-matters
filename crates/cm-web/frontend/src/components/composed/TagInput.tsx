import { useCallback, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";

export interface TagInputProps {
  /** Current tags */
  value: string[];
  /** Called with updated tag array */
  onChange: (tags: string[]) => void;
  /** Available suggestions (excluding current tags) */
  suggestions?: string[];
  /** Input placeholder when no tags present */
  placeholder?: string;
  /** Max visible suggestions in dropdown */
  maxSuggestions?: number;
}

export function TagInput({
  value,
  onChange,
  suggestions = [],
  placeholder = "Add tags...",
  maxSuggestions = 8,
}: TagInputProps) {
  const [input, setInput] = useState("");
  const [showSuggestions, setShowSuggestions] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  const availableSuggestions = useMemo(
    () => suggestions.filter((s) => !value.includes(s)),
    [suggestions, value],
  );

  const filteredSuggestions = useMemo(() => {
    if (!input.trim()) return availableSuggestions.slice(0, maxSuggestions);
    const query = input.toLowerCase();
    return availableSuggestions
      .filter((s) => s.toLowerCase().includes(query))
      .slice(0, maxSuggestions);
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
            className="absolute left-0 top-full z-50 mt-1 max-h-[160px] w-[200px] overflow-y-auto rounded-md border border-border bg-popover p-1 shadow-md"
          >
            {filteredSuggestions.map((suggestion) => (
              <button
                key={suggestion}
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleAdd(suggestion)}
                className="block w-full rounded-sm px-2 py-1 text-left font-mono text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground"
              >
                {suggestion}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
