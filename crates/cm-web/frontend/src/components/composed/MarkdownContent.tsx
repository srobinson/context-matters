import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";

interface MarkdownContentProps {
  children: string;
  className?: string;
}

interface FrontmatterField {
  key: string;
  value: string;
}

function isExternalHref(href?: string) {
  return typeof href === "string" && /^(https?:)?\/\//.test(href);
}

function extractLeadingFrontmatter(source: string) {
  if (!source.startsWith("---\n") && !source.startsWith("---\r\n")) {
    return { fields: null, raw: null, body: source };
  }

  const lines = source.split(/\r?\n/);
  if (lines[0] !== "---") {
    return { fields: null, raw: null, body: source };
  }

  let closingIndex = -1;
  for (let i = 1; i < lines.length; i += 1) {
    if (lines[i]?.trim() === "---") {
      closingIndex = i;
      break;
    }
  }

  if (closingIndex === -1) {
    return { fields: null, raw: null, body: source };
  }

  const rawLines = lines.slice(1, closingIndex);
  const body = lines
    .slice(closingIndex + 1)
    .join("\n")
    .replace(/^\n+/, "");
  const fields = parseFrontmatterFields(rawLines);

  return {
    fields: fields.length > 0 ? fields : null,
    raw: rawLines.join("\n"),
    body,
  };
}

function parseFrontmatterFields(lines: string[]): FrontmatterField[] {
  const fields: FrontmatterField[] = [];
  let current: FrontmatterField | null = null;

  for (const line of lines) {
    if (!line.trim()) {
      if (current) {
        current.value = `${current.value}\n`;
      }
      continue;
    }

    const match = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (match) {
      const field: FrontmatterField = {
        key: match[1] ?? "",
        value: match[2] ?? "",
      };
      current = field;
      fields.push(field);
      continue;
    }

    if (current && (line.startsWith("  ") || line.startsWith("\t") || line.startsWith("- "))) {
      current.value = current.value ? `${current.value}\n${line.trim()}` : line.trim();
      continue;
    }

    return [];
  }

  return fields.map((field) => ({
    ...field,
    value: field.value.trim(),
  }));
}

function FrontmatterPanel({
  fields,
  raw,
}: {
  fields: FrontmatterField[] | null;
  raw: string | null;
}) {
  if (fields && fields.length > 0) {
    return (
      <section className="not-prose rounded-lg border border-border/80 bg-muted/40 p-3">
        <p className="mb-3 font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/70">
          frontmatter
        </p>
        <dl className="grid gap-x-4 gap-y-2 sm:grid-cols-[minmax(0,8rem)_minmax(0,1fr)]">
          {fields.map((field) => (
            <div key={field.key} className="contents">
              <dt className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
                {field.key}
              </dt>
              <dd className="whitespace-pre-wrap break-words font-mono text-xs text-foreground/88">
                {field.value || "—"}
              </dd>
            </div>
          ))}
        </dl>
      </section>
    );
  }

  if (raw?.trim()) {
    return (
      <section className="not-prose rounded-lg border border-border/80 bg-muted/40 p-3">
        <p className="mb-3 font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/70">
          frontmatter
        </p>
        <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-xs leading-relaxed text-foreground/80">
          {raw}
        </pre>
      </section>
    );
  }

  return null;
}

export function MarkdownContent({ children, className }: MarkdownContentProps) {
  const { fields, raw, body } = extractLeadingFrontmatter(children);

  return (
    <div className={cn("space-y-4", className)}>
      <FrontmatterPanel fields={fields} raw={raw} />

      <div className="prose prose-sm max-w-none prose-neutral dark:prose-invert prose-headings:font-sans prose-headings:font-medium prose-headings:tracking-tight prose-headings:text-foreground prose-p:text-foreground/88 prose-p:leading-7 prose-strong:text-foreground prose-a:text-foreground prose-a:underline prose-a:decoration-muted-foreground/40 prose-a:underline-offset-4 hover:prose-a:decoration-foreground/70 prose-ul:my-4 prose-ol:my-4 prose-li:my-1 prose-li:text-foreground/88 prose-hr:border-border prose-blockquote:border-l-border prose-blockquote:text-muted-foreground prose-pre:rounded-md prose-pre:border prose-pre:border-border prose-pre:bg-muted/72 prose-pre:px-4 prose-pre:py-3 prose-pre:text-foreground prose-code:rounded prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:font-mono prose-code:text-[0.875em] prose-code:text-foreground prose-code:before:content-none prose-code:after:content-none prose-th:border prose-th:border-border prose-th:bg-muted/72 prose-th:px-3 prose-th:py-2 prose-th:text-left prose-td:border prose-td:border-border prose-td:px-3 prose-td:py-2 dark:prose-pre:bg-muted/60 dark:prose-code:bg-muted/80">
        <Markdown
          remarkPlugins={[remarkGfm]}
          components={{
            a: ({ href, ...props }) => (
              <a
                {...props}
                href={href}
                target={isExternalHref(href) ? "_blank" : undefined}
                rel={isExternalHref(href) ? "noreferrer" : undefined}
              />
            ),
          }}
        >
          {body}
        </Markdown>
      </div>
    </div>
  );
}
