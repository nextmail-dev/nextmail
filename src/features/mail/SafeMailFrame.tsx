import { useEffect, useMemo, useState } from "react";

import {
  DARK_MAIL_SURFACE,
  harmonizeLightMailDocument,
  hasAuthoredDarkMode,
  LIGHT_MAIL_SURFACE,
  smartInvertMailDocument,
} from "./mail-dark-mode";

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
  if (dark && !hasAuthoredDarkMode(document)) {
    document = smartInvertMailDocument(document);
  } else if (!dark) {
    document = harmonizeLightMailDocument(document);
  }
  const themeStyle = dark
    ? `<style id="nextmail-reader-theme">html{color-scheme:dark;background:${DARK_MAIL_SURFACE};color:#e8e8e8}body{background:${DARK_MAIL_SURFACE};color:#e8e8e8}a{color:#8ab4f8}*{border-color:#6f6f6f}</style>`
    : `<style id="nextmail-reader-theme">html{color-scheme:light;background:${LIGHT_MAIL_SURFACE};color:#202124}body{background:${LIGHT_MAIL_SURFACE};color:#202124}</style>`;
  document = /<head(?:\s[^>]*)?>/i.test(document)
    ? document.replace(/<head(\s[^>]*)?>/i, (head) => `${head}${themeStyle}`)
    : `${themeStyle}${document}`;
  return document;
}
