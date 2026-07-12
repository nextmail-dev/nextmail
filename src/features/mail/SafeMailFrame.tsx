export function SafeMailFrame({ document, title }: { document: string; title: string }) {
  return (
    <iframe
      className="size-full border-0 bg-white"
      title={title}
      sandbox=""
      referrerPolicy="no-referrer"
      srcDoc={document}
    />
  );
}
