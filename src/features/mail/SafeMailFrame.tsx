import { useEffect, useMemo, useState } from "react";

interface SafeMailFrameProps {
  document: string;
  title: string;
  allowRemoteImages?: boolean;
}

export function SafeMailFrame({ document, title, allowRemoteImages = false }: SafeMailFrameProps) {
  const systemDark = useSystemDarkMode();
  const theme = globalThis.document?.documentElement.dataset.theme;
  const dark = theme === "dark" || (theme === "system" && systemDark);
  const source = useMemo(
    () => prepareFrameDocument(document, allowRemoteImages, dark),
    [allowRemoteImages, dark, document],
  );
  return (
    <iframe
      className="size-full border-0 bg-card"
      title={title}
      sandbox="allow-popups"
      referrerPolicy="no-referrer"
      style={{ colorScheme: dark ? "dark" : "light" }}
      srcDoc={source}
    />
  );
}

function useSystemDarkMode() {
  const query = useMemo(
    () => typeof window === "undefined" || !window.matchMedia
      ? null
      : window.matchMedia("(prefers-color-scheme: dark)"),
    [],
  );
  const [dark, setDark] = useState(query?.matches ?? false);
  useEffect(() => {
    if (!query) return;
    const update = (event: MediaQueryListEvent) => setDark(event.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, [query]);
  return dark;
}

function prepareFrameDocument(source: string, allowRemoteImages: boolean, dark: boolean) {
  let document = allowRemoteImages
    ? source.replace("img-src data:;", "img-src data: http: https:;")
    : source;
  const themeStyle = dark
    ? `<style id="nextmail-reader-theme">html{color-scheme:dark;background:#181818;color:#e8e8e8}body{background:#181818;color:#e8e8e8}a{color:#8ab4f8}hr,table{border-color:#3a3a3a}</style>`
    : `<style id="nextmail-reader-theme">html{color-scheme:light;background:#fff;color:#202124}body{background:#fff;color:#202124}</style>`;
  document = document.includes("</head>")
    ? document.replace("</head>", `${themeStyle}</head>`)
    : `${themeStyle}${document}`;
  return document;
}
