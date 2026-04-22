import { Search, X } from "lucide-react";
import type { RefObject } from "react";
import { Input } from "@/components/ui/input";

interface FeedSearchInputProps {
  inputRef: RefObject<HTMLInputElement | null>;
  isRecallMode: boolean;
  searchInput: string;
  onSearchInputChange: (value: string) => void;
  onClearSearch: () => void;
}

export function FeedSearchInput({
  inputRef,
  isRecallMode,
  searchInput,
  onSearchInputChange,
  onClearSearch,
}: FeedSearchInputProps) {
  return (
    <div className="relative">
      <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        ref={inputRef}
        type="text"
        placeholder={
          isRecallMode
            ? "Recall query (matches cx_recall)..."
            : "Switch to Recall mode to search like MCP..."
        }
        value={searchInput}
        onChange={(event) => onSearchInputChange(event.target.value)}
        disabled={!isRecallMode}
        className="pl-8 pr-8 font-mono text-xs"
      />
      {isRecallMode && searchInput && (
        <button
          type="button"
          onClick={onClearSearch}
          className="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}
