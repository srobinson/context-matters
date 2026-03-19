import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export type FeedMode = "default" | "recall";

export function FeedModeSelect({
  value,
  onChange,
}: {
  value: FeedMode;
  onChange: (mode: FeedMode) => void;
}) {
  return (
    <Select value={value} onValueChange={(next) => onChange(next as FeedMode)}>
      <SelectTrigger className="h-7 w-[128px] gap-1 rounded-md border-border bg-muted px-2 font-mono text-xs text-muted-foreground">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="default">Default</SelectItem>
        <SelectItem value="recall">Recall</SelectItem>
      </SelectContent>
    </Select>
  );
}
