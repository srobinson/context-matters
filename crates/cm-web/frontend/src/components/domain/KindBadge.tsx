import type { EntryKind } from "@/api/generated/EntryKind";
import { cn } from "@/lib/utils";

const KIND_STYLES: Record<EntryKind, string> = {
  fact: "bg-[var(--color-kind-fact)]/15 dark:bg-[var(--color-kind-fact)]/20 text-[var(--color-kind-fact)] border-[var(--color-kind-fact)]/25 dark:border-[var(--color-kind-fact)]/35",
  decision:
    "bg-[var(--color-kind-decision)]/15 dark:bg-[var(--color-kind-decision)]/20 text-[var(--color-kind-decision)] border-[var(--color-kind-decision)]/25 dark:border-[var(--color-kind-decision)]/35",
  preference:
    "bg-[var(--color-kind-preference)]/15 dark:bg-[var(--color-kind-preference)]/20 text-[var(--color-kind-preference)] border-[var(--color-kind-preference)]/25 dark:border-[var(--color-kind-preference)]/35",
  lesson:
    "bg-[var(--color-kind-lesson)]/15 dark:bg-[var(--color-kind-lesson)]/20 text-[var(--color-kind-lesson)] border-[var(--color-kind-lesson)]/25 dark:border-[var(--color-kind-lesson)]/35",
  reference:
    "bg-[var(--color-kind-reference)]/15 dark:bg-[var(--color-kind-reference)]/20 text-[var(--color-kind-reference)] border-[var(--color-kind-reference)]/25 dark:border-[var(--color-kind-reference)]/35",
  feedback:
    "bg-[var(--color-kind-feedback)]/15 dark:bg-[var(--color-kind-feedback)]/20 text-[var(--color-kind-feedback)] border-[var(--color-kind-feedback)]/25 dark:border-[var(--color-kind-feedback)]/35",
  pattern:
    "bg-[var(--color-kind-pattern)]/15 dark:bg-[var(--color-kind-pattern)]/20 text-[var(--color-kind-pattern)] border-[var(--color-kind-pattern)]/25 dark:border-[var(--color-kind-pattern)]/35",
  observation:
    "bg-[var(--color-kind-observation)]/15 dark:bg-[var(--color-kind-observation)]/20 text-[var(--color-kind-observation)] border-[var(--color-kind-observation)]/25 dark:border-[var(--color-kind-observation)]/35",
};

export function KindBadge({ kind, className }: { kind: EntryKind; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-medium leading-none",
        KIND_STYLES[kind],
        className,
      )}
    >
      {kind}
    </span>
  );
}
