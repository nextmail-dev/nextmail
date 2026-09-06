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
      className="block size-full border-0 bg-card"
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

  const mailDocument = new DOMParser().parseFromString(document, "text/html");
  // Keep the scriptless, opaque frame: style its own scrollbar instead of
  // reading its DOM from OverlayScrollArea. The 6px track fits in the existing inset.
  const scrollbarForeground = getComputedStyle(globalThis.document.documentElement)
    .getPropertyValue("--muted-foreground").trim() || "currentColor";
  const readerStyle = mailDocument.createElement("style");
  readerStyle.id = "nextmail-reader-scrollbar";
  readerStyle.textContent = `
    :root { min-height:100%; overflow:auto; scrollbar-color:auto; scrollbar-width:auto; }
    :root > body { box-sizing:border-box; width:calc(100vw - 16px) !important; margin:0 0 0 8px !important; }
    :root::-webkit-scrollbar { width:6px; height:6px; background:transparent; cursor:default; }
    :root::-webkit-scrollbar-thumb { min-height:32px; border-radius:999px; background:color-mix(in srgb, ${scrollbarForeground} 55%, transparent); cursor:default; }
    :root::-webkit-scrollbar-thumb:hover { background:color-mix(in srgb, ${scrollbarForeground} 70%, transparent); }
    :root::-webkit-scrollbar-track, :root::-webkit-scrollbar-corner { background:transparent; }
    :root::-webkit-scrollbar-button { display:none; width:0; height:0; }
  `;
  mailDocument.head.append(readerStyle);
  for (const image of mailDocument.querySelectorAll<HTMLImageElement>("img")) {
    image.style.setProperty("max-width", "calc(100vw - 16px)", "important");
    image.style.setProperty("box-sizing", "border-box", "important");
    image.style.setProperty("height", "auto", "important");
  }
  return `<!doctype html>${mailDocument.documentElement.outerHTML}`;
}
