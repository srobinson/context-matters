import type { BrowseSort } from "@/api/generated/BrowseSort";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const SORT_OPTIONS: { value: BrowseSort; label: string }[] = [
  { value: "recent", label: "Recent" },
  { value: "oldest", label: "Oldest" },
  { value: "title_asc", label: "Title A-Z" },
  { value: "title_desc", label: "Title Z-A" },
  { value: "scope_asc", label: "Scope A-Z" },
  { value: "scope_desc", label: "Scope Z-A" },
  { value: "kind_asc", label: "Kind A-Z" },
  { value: "kind_desc", label: "Kind Z-A" },
];

export function SortSelect({
  value,
  onChange,
}: {
  value: BrowseSort;
  onChange: (sort: BrowseSort) => void;
}) {
  return (
    <Select value={value} onValueChange={(v) => onChange(v as BrowseSort)}>
      <SelectTrigger className="h-7 w-[130px] gap-1 rounded-md border-border bg-muted px-2 font-mono text-xs text-muted-foreground">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {SORT_OPTIONS.map((opt) => (
          <SelectItem key={opt.value} value={opt.value}>
            {opt.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
