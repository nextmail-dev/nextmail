export type DesktopPlatform = "windows" | "macos" | "other";

export function detectDesktopPlatform(userAgent = globalThis.navigator?.userAgent ?? ""): DesktopPlatform {
  if (/Windows NT/i.test(userAgent)) return "windows";
  if (/Macintosh|Mac OS X/i.test(userAgent)) return "macos";
  return "other";
}

export function applyDesktopPlatform(target: Document = document) {
  target.documentElement.dataset.platform = detectDesktopPlatform();
}
