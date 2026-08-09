import Markdown, { type Components } from "react-markdown";

const allowedElements = [
  "a",
  "blockquote",
  "br",
  "code",
  "em",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "hr",
  "li",
  "ol",
  "p",
  "pre",
  "strong",
  "ul",
];

const components: Components = {
  h1: ({ children }) => <h1 className="mt-0 mb-3 text-xl font-semibold">{children}</h1>,
  h2: ({ children }) => <h2 className="mt-5 mb-2 text-lg font-semibold first:mt-0">{children}</h2>,
  h3: ({ children }) => <h3 className="mt-4 mb-2 text-base font-semibold first:mt-0">{children}</h3>,
  h4: ({ children }) => <h4 className="mt-3 mb-1.5 text-sm font-semibold">{children}</h4>,
  h5: ({ children }) => <h5 className="mt-3 mb-1.5 text-sm font-semibold">{children}</h5>,
  h6: ({ children }) => <h6 className="mt-3 mb-1.5 text-sm font-semibold">{children}</h6>,
  p: ({ children }) => <p className="my-2 leading-6 first:mt-0 last:mb-0">{children}</p>,
  ul: ({ children }) => <ul className="my-2 list-disc space-y-1 pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="my-2 list-decimal space-y-1 pl-5">{children}</ol>,
  li: ({ children }) => <li className="pl-0.5">{children}</li>,
  blockquote: ({ children }) => (
    <blockquote className="my-3 border-l-3 border-primary/35 pl-3 text-muted-foreground">{children}</blockquote>
  ),
  hr: () => <hr className="my-4 border-0 border-t border-border" />,
  pre: ({ children }) => <pre className="my-3 overflow-x-auto rounded-md bg-background/80 p-3 text-xs">{children}</pre>,
  code: ({ children }) => <code className="rounded bg-background/80 px-1 py-0.5 font-mono text-[0.9em]">{children}</code>,
  a: ({ href, children }) => {
    const safeHref = safeExternalUrl(href);
    return safeHref ? (
      <a
        className="font-medium text-primary underline decoration-primary/35 underline-offset-2 hover:decoration-primary"
        href={safeHref}
        target="_blank"
        rel="noreferrer noopener"
      >
        {children}
      </a>
    ) : <span>{children}</span>;
  },
};

export function ReleaseNotesMarkdown({ children }: { children: string }) {
  return (
    <div className="select-text text-sm leading-6 text-foreground">
      <Markdown
        allowedElements={allowedElements}
        components={components}
        skipHtml
      >
        {children}
      </Markdown>
    </div>
  );
}

function safeExternalUrl(candidate?: string) {
  if (!candidate) return null;
  try {
    const url = new URL(candidate);
    if ((url.protocol !== "http:" && url.protocol !== "https:") || url.username || url.password) {
      return null;
    }
    return url.toString();
  } catch {
    return null;
  }
}
