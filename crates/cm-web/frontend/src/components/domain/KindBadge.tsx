import type { EntryKind } from "@/api/generated/EntryKind";
import { cn } from "@/lib/utils";

const KIND_STYLES: Record<EntryKind, string> = {
  fact: "bg-[var(--color-kind-fact)]/15 text-[var(--color-kind-fact)] border-[var(--color-kind-fact)]/25",
  decision: "bg-[var(--color-kind-decision)]/15 text-[var(--color-kind-decision)] border-[var(--color-kind-decision)]/25",
  preference: "bg-[var(--color-kind-preference)]/15 text-[var(--color-kind-preference)] border-[var(--color-kind-preference)]/25",
  lesson: "bg-[var(--color-kind-lesson)]/15 text-[var(--color-kind-lesson)] border-[var(--color-kind-lesson)]/25",
  reference: "bg-[var(--color-kind-reference)]/15 text-[var(--color-kind-reference)] border-[var(--color-kind-reference)]/25",
  feedback: "bg-[var(--color-kind-feedback)]/15 text-[var(--color-kind-feedback)] border-[var(--color-kind-feedback)]/25",
  pattern: "bg-[var(--color-kind-pattern)]/15 text-[var(--color-kind-pattern)] border-[var(--color-kind-pattern)]/25",
  observation: "bg-[var(--color-kind-observation)]/15 text-[var(--color-kind-observation)] border-[var(--color-kind-observation)]/25",
};

export function KindBadge({
  kind,
  className,
}: {
  kind: EntryKind;
  className?: string;
}) {
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
