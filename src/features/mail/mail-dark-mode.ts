const MAX_MAIL_ELEMENTS = 10_000;
const MAX_MAIL_STYLE_RULES = 2_048;
const INLINE_SPECIFICITY = 1_000_000_000;
const TARGET_CONTRAST = 4.55;
const TARGET_BORDER_CONTRAST = 3.05;
const MAX_DARK_SURFACE_LUMINANCE = 0.12;
const DARK_LIGHTNESS_FLOOR = 0.09;
const DARK_LIGHTNESS_CEILING = 0.9;

const COLOR_PROPERTIES = [
  "color",
  "background-color",
  "border-top-color",
  "border-right-color",
  "border-bottom-color",
  "border-left-color",
] as const;

type ColorProperty = (typeof COLOR_PROPERTIES)[number];

interface Candidate {
  value: string;
  important: boolean;
  specificity: number;
  order: number;
}

interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

interface Hsla {
  h: number;
  s: number;
  l: number;
  a: number;
}

interface ElementColorState {
  originalForeground: Rgba;
  originalBackground: Rgba;
  effectiveBackground: Rgba;
  backgroundOwner: HTMLElement | null;
  darkBackground: Rgba | null;
  parentEffectiveBackground: Rgba;
  authoredBackground: boolean;
}

type Cascade = Map<HTMLElement, Partial<Record<ColorProperty, Candidate>>>;

const LIGHT_FOREGROUND: Rgba = { r: 32, g: 33, b: 36, a: 1 };
const LIGHT_BACKGROUND: Rgba = { r: 255, g: 255, b: 255, a: 1 };
export const DARK_MAIL_SURFACE = "#171717";
const DARK_READER_BACKGROUND: Rgba = { r: 23, g: 23, b: 23, a: 1 };
const TRANSPARENT: Rgba = { r: 0, g: 0, b: 0, a: 0 };

export function smartInvertMailDocument(source: string) {
  if (typeof DOMParser === "undefined" || typeof globalThis.document === "undefined") return source;
  const mailDocument = new DOMParser().parseFromString(source, "text/html");
  const elements = Array.from(mailDocument.querySelectorAll<HTMLElement>("html, body, body *"));
  if (!elements.length || elements.length > MAX_MAIL_ELEMENTS) return source;

  const cascade: Cascade = new Map();
  applyPresentationalColors(elements, cascade);
  if (!applyStyleSheetColors(mailDocument, cascade)) return source;
  applyInlineColors(elements, cascade);

  const parser = createColorParser();
  if (!parser) return source;
  try {
    writeDarkColors(mailDocument, elements, cascade, parser.parse);
  } finally {
    parser.dispose();
  }
  return `<!doctype html>${mailDocument.documentElement.outerHTML}`;
}

export function hasAuthoredDarkMode(source: string) {
  return /\bdata-nextmail-native-dark(?:\s*=|\s|>)/i.test(source);
}

function applyPresentationalColors(elements: HTMLElement[], cascade: Cascade) {
  for (const element of elements) {
    const background = element.getAttribute("bgcolor");
    if (background) setCandidate(cascade, element, "background-color", {
      value: background,
      important: false,
      specificity: 0,
      order: -1,
    });
    if (element.localName === "font") {
      const color = element.getAttribute("color");
      if (color) setCandidate(cascade, element, "color", {
        value: color,
        important: false,
        specificity: 0,
        order: -1,
      });
    }
    const border = element.getAttribute("bordercolor")
      ?? (element.localName === "hr" ? element.getAttribute("color") : null);
    if (border) {
      for (const property of COLOR_PROPERTIES.slice(2) as readonly ColorProperty[]) {
        setCandidate(cascade, element, property, {
          value: border,
          important: false,
          specificity: 0,
          order: -1,
        });
      }
    }
  }
}

function applyStyleSheetColors(mailDocument: Document, cascade: Cascade) {
  let order = 0;
  let ruleCount = 0;
  for (const styleElement of mailDocument.querySelectorAll("style")) {
    const rules = parseStyleRules(styleElement.textContent ?? "");
    const applyRules = (values: CSSRule[]) => {
      for (const rule of values) {
        const styleRule = rule as CSSStyleRule;
        if (typeof styleRule.selectorText === "string" && styleRule.style) {
          ruleCount += 1;
          if (ruleCount > MAX_MAIL_STYLE_RULES) return false;
          order += 1;
          for (const selector of splitSelectorList(styleRule.selectorText)) {
            let matches: NodeListOf<Element>;
            try {
              matches = mailDocument.querySelectorAll(selector);
            } catch {
              continue;
            }
            const specificity = selectorSpecificity(selector);
            for (const match of matches) {
              if (!(match instanceof HTMLElement)) continue;
              for (const property of COLOR_PROPERTIES) {
                const value = styleRule.style.getPropertyValue(property).trim();
                if (!value) continue;
                setCandidate(cascade, match, property, {
                  value,
                  important: styleRule.style.getPropertyPriority(property) === "important",
                  specificity,
                  order,
                });
              }
            }
          }
          continue;
        }

        const mediaRule = rule as CSSMediaRule;
        if (mediaRule.cssRules && typeof mediaRule.conditionText === "string") {
          ruleCount += 1;
          if (ruleCount > MAX_MAIL_STYLE_RULES) return false;
          if (mailMediaMatches(mediaRule.conditionText) && !applyRules(Array.from(mediaRule.cssRules))) {
            return false;
          }
        }
      }
      return true;
    };
    if (!applyRules(rules)) return false;
  }
  return true;
}

function parseStyleRules(source: string) {
  if (!source.trim()) return [];
  const owner = globalThis.document.implementation.createHTMLDocument("");
  const style = owner.createElement("style");
  style.textContent = source;
  owner.head.append(style);
  try {
    return style.sheet ? Array.from(style.sheet.cssRules) : [];
  } catch {
    return [];
  }
}

function mailMediaMatches(query: string) {
  if (/prefers-color-scheme\s*:\s*dark/i.test(query)) return false;
  const branches = query.split(",").map((branch) => branch
    .split(/\band\b/i)
    .map((part) => part.trim())
    .filter((part) => part && !/prefers-color-scheme\s*:\s*light/i.test(part))
    .join(" and "));
  const remaining = branches.map((branch) => branch || "all").join(", ");
  if (typeof globalThis.matchMedia !== "function") {
    return branches.every((branch) => !branch || /^(?:all|screen)$/i.test(branch));
  }
  try {
    return globalThis.matchMedia(remaining).matches;
  } catch {
    return false;
  }
}

function applyInlineColors(elements: HTMLElement[], cascade: Cascade) {
  let order = 1_000_000;
  for (const element of elements) {
    order += 1;
    for (const property of COLOR_PROPERTIES) {
      const value = element.style.getPropertyValue(property).trim();
      if (!value) continue;
      setCandidate(cascade, element, property, {
        value,
        important: element.style.getPropertyPriority(property) === "important",
        specificity: INLINE_SPECIFICITY,
        order,
      });
    }
  }
}

function setCandidate(
  cascade: Cascade,
  element: HTMLElement,
  property: ColorProperty,
  candidate: Candidate,
) {
  const values = cascade.get(element) ?? {};
  const current = values[property];
  if (!current || winsCascade(candidate, current)) values[property] = candidate;
  cascade.set(element, values);
}

function winsCascade(next: Candidate, current: Candidate) {
  if (next.important !== current.important) return next.important;
  if (next.specificity !== current.specificity) return next.specificity > current.specificity;
  return next.order >= current.order;
}

function writeDarkColors(
  mailDocument: Document,
  elements: HTMLElement[],
  cascade: Cascade,
  parseColor: (value: string) => Rgba | null,
) {
  const states = new Map<HTMLElement, ElementColorState>();

  for (const element of elements) {
    const parent = element.parentElement ? states.get(element.parentElement) : undefined;
    const currentParentSurface = parent?.backgroundOwner
      ? states.get(parent.backgroundOwner)?.effectiveBackground
      : parent?.effectiveBackground;
    const parentForeground = parent?.originalForeground ?? LIGHT_FOREGROUND;
    const parentOriginalBackground = parent?.originalBackground ?? LIGHT_BACKGROUND;
    const parentEffectiveBackground = currentParentSurface ?? DARK_READER_BACKGROUND;
    const candidates = cascade.get(element) ?? {};
    const foreground = resolveColor(
      candidates.color?.value,
      "color",
      parentForeground,
      parentOriginalBackground,
      parentForeground,
      parseColor,
    );
    const rootDefault = element === mailDocument.documentElement ? LIGHT_BACKGROUND : TRANSPARENT;
    const background = resolveColor(
      candidates["background-color"]?.value,
      "background-color",
      parentForeground,
      parentOriginalBackground,
      rootDefault,
      parseColor,
    );

    if (element.localName === "img" || element.localName === "video") {
      states.set(element, {
        originalForeground: foreground,
        originalBackground: background,
        effectiveBackground: parentEffectiveBackground,
        backgroundOwner: parent?.backgroundOwner ?? null,
        darkBackground: null,
        parentEffectiveBackground,
        authoredBackground: false,
      });
      continue;
    }

    const ownsBackground = Boolean(candidates["background-color"])
      || element === mailDocument.documentElement;
    const darkBackground = adaptDarkSurface(background, parentEffectiveBackground);
    const effectiveBackground = composite(darkBackground, parentEffectiveBackground);
    const state: ElementColorState = {
      originalForeground: foreground,
      originalBackground: background,
      effectiveBackground: ownsBackground ? effectiveBackground : parentEffectiveBackground,
      backgroundOwner: ownsBackground ? element : (parent?.backgroundOwner ?? null),
      darkBackground: ownsBackground ? darkBackground : null,
      parentEffectiveBackground,
      authoredBackground: Boolean(candidates["background-color"]),
    };
    states.set(element, state);

    const preferredForeground = invertLightness(foreground);
    const owner = state.backgroundOwner ? states.get(state.backgroundOwner) : undefined;
    if (owner?.authoredBackground && owner.darkBackground && isChromatic(owner.originalBackground)) {
      owner.darkBackground = darkenSurfaceForForeground(
        owner.darkBackground,
        owner.parentEffectiveBackground,
        preferredForeground,
      );
      owner.effectiveBackground = composite(owner.darkBackground, owner.parentEffectiveBackground);
      state.backgroundOwner?.style.setProperty(
        "background-color",
        serializeColor(owner.darkBackground),
        "important",
      );
    }
    state.effectiveBackground = owner?.effectiveBackground ?? state.effectiveBackground;
    const darkForeground = ensureContrast(preferredForeground, state.effectiveBackground, TARGET_CONTRAST);
    const shouldWriteForeground = Boolean(candidates.color)
      || Boolean(candidates["background-color"])
      || element === mailDocument.documentElement
      || element === mailDocument.body;
    if (shouldWriteForeground) {
      element.style.setProperty("color", serializeColor(darkForeground), "important");
    }
    if (candidates["background-color"] || element === mailDocument.documentElement) {
      element.style.setProperty(
        "background-color",
        serializeColor(state.darkBackground ?? darkBackground),
        "important",
      );
    }

    for (const property of COLOR_PROPERTIES.slice(2) as readonly ColorProperty[]) {
      const candidate = candidates[property];
      if (!candidate) continue;
      const border = resolveColor(
        candidate.value,
        property,
        foreground,
        parentOriginalBackground,
        foreground,
        parseColor,
      );
      element.style.setProperty(
        property,
        serializeColor(ensureContrast(invertLightness(border), state.effectiveBackground, TARGET_BORDER_CONTRAST)),
        "important",
      );
    }
  }
}

function resolveColor(
  value: string | undefined,
  property: ColorProperty,
  inheritedForeground: Rgba,
  inheritedBackground: Rgba,
  initial: Rgba,
  parseColor: (value: string) => Rgba | null,
) {
  if (!value) return property === "color" ? inheritedForeground : initial;
  const normalized = value.trim().toLowerCase();
  if (normalized === "currentcolor") return inheritedForeground;
  if (normalized === "inherit") {
    return property === "color" ? inheritedForeground : inheritedBackground;
  }
  if (matchesCssWideKeyword(normalized)) {
    return property === "color" && normalized !== "initial" ? inheritedForeground : initial;
  }
  return parseColor(value) ?? (property === "color" ? inheritedForeground : initial);
}

function matchesCssWideKeyword(value: string) {
  return matches(value, ["initial", "unset", "revert", "revert-layer"]);
}

function createColorParser() {
  const owner = globalThis.document;
  const probe = owner.createElement("span");
  probe.hidden = true;
  probe.style.position = "fixed";
  probe.style.pointerEvents = "none";
  owner.documentElement.append(probe);
  const cache = new Map<string, Rgba | null>();
  return {
    parse(value: string) {
      const cached = cache.get(value);
      if (cached !== undefined) return cached;
      probe.style.color = "";
      probe.style.color = value;
      const parsed = probe.style.color
        ? parseResolvedColor(globalThis.getComputedStyle(probe).color)
        : null;
      cache.set(value, parsed);
      return parsed;
    },
    dispose() {
      probe.remove();
    },
  };
}

function parseResolvedColor(value: string): Rgba | null {
  const rgb = value.match(/^rgba?\((.*)\)$/i);
  if (rgb) {
    const [channels, slashAlpha] = rgb[1].split("/").map((part) => part.trim());
    const values = channels.replace(/,/g, " ").split(/\s+/).filter(Boolean);
    const alphaValue = slashAlpha ?? (values.length === 4 ? values.pop() : undefined);
    if (values.length !== 3) return null;
    const parsed = values.map(parseRgbChannel);
    const alpha = alphaValue === undefined ? 1 : parseAlpha(alphaValue);
    if (parsed.some((channel) => channel === null) || alpha === null) return null;
    return { r: parsed[0]!, g: parsed[1]!, b: parsed[2]!, a: alpha };
  }

  const srgb = value.match(/^color\(srgb\s+([^)]*)\)$/i);
  if (srgb) {
    const [channels, alphaValue] = srgb[1].split("/").map((part) => part.trim());
    const values = channels.split(/\s+/).map(Number);
    const alpha = alphaValue === undefined ? 1 : parseAlpha(alphaValue);
    if (values.length !== 3 || values.some((channel) => !Number.isFinite(channel)) || alpha === null) {
      return null;
    }
    return {
      r: clamp(values[0], 0, 1) * 255,
      g: clamp(values[1], 0, 1) * 255,
      b: clamp(values[2], 0, 1) * 255,
      a: alpha,
    };
  }
  return null;
}

function parseRgbChannel(value: string) {
  if (value.endsWith("%")) {
    const percent = Number.parseFloat(value);
    return Number.isFinite(percent) ? clamp(percent, 0, 100) * 2.55 : null;
  }
  const channel = Number.parseFloat(value);
  return Number.isFinite(channel) ? clamp(channel, 0, 255) : null;
}

function parseAlpha(value: string) {
  if (value.endsWith("%")) {
    const percent = Number.parseFloat(value);
    return Number.isFinite(percent) ? clamp(percent / 100, 0, 1) : null;
  }
  const alpha = Number.parseFloat(value);
  return Number.isFinite(alpha) ? clamp(alpha, 0, 1) : null;
}

export function invertLightness(color: Rgba): Rgba {
  const hsl = rgbToHsl(color);
  return hslToRgb({
    ...hsl,
    l: DARK_LIGHTNESS_FLOOR + (1 - hsl.l) * (DARK_LIGHTNESS_CEILING - DARK_LIGHTNESS_FLOOR),
  });
}

function adaptDarkSurface(color: Rgba, parentBackground: Rgba) {
  const inverted = invertLightness(color);
  if (inverted.a <= 0 || relativeLuminance(composite(inverted, parentBackground)) <= MAX_DARK_SURFACE_LUMINANCE) {
    return inverted;
  }

  const hsl = rgbToHsl(inverted);
  let lower = 0;
  let upper = hsl.l;
  let best = hslToRgb({ ...hsl, l: 0 });
  for (let index = 0; index < 14; index += 1) {
    const lightness = (lower + upper) / 2;
    const candidate = hslToRgb({ ...hsl, l: lightness });
    if (relativeLuminance(composite(candidate, parentBackground)) <= MAX_DARK_SURFACE_LUMINANCE) {
      best = candidate;
      lower = lightness;
    } else {
      upper = lightness;
    }
  }
  return best;
}

function darkenSurfaceForForeground(surface: Rgba, parentBackground: Rgba, foreground: Rgba) {
  if (contrastRatio(foreground, composite(surface, parentBackground)) >= TARGET_CONTRAST) {
    return surface;
  }

  const hsl = rgbToHsl(surface);
  const darkest = hslToRgb({ ...hsl, l: 0 });
  if (contrastRatio(foreground, composite(darkest, parentBackground)) < TARGET_CONTRAST) {
    return surface;
  }

  let lower = 0;
  let upper = hsl.l;
  let best = darkest;
  for (let index = 0; index < 14; index += 1) {
    const lightness = (lower + upper) / 2;
    const candidate = hslToRgb({ ...hsl, l: lightness });
    if (contrastRatio(foreground, composite(candidate, parentBackground)) >= TARGET_CONTRAST) {
      best = candidate;
      lower = lightness;
    } else {
      upper = lightness;
    }
  }
  return best;
}

function isChromatic(color: Rgba) {
  return color.a > 0 && rgbToHsl(color).s >= 0.08;
}

export function contrastRatio(foreground: Rgba, background: Rgba) {
  const flattenedForeground = composite(foreground, background);
  const light = Math.max(relativeLuminance(flattenedForeground), relativeLuminance(background));
  const dark = Math.min(relativeLuminance(flattenedForeground), relativeLuminance(background));
  return (light + 0.05) / (dark + 0.05);
}

function ensureContrast(foreground: Rgba, background: Rgba, target: number) {
  if (contrastRatio(foreground, background) >= target) return foreground;
  const hsl = rgbToHsl(foreground);
  const black = hslToRgb({ ...hsl, l: 0, a: 1 });
  const white = hslToRgb({ ...hsl, l: 1, a: 1 });
  const endpoint = contrastRatio(white, background) >= contrastRatio(black, background) ? 1 : 0;
  let failing = hsl.l;
  let passing = endpoint;
  let best = hslToRgb({ ...hsl, l: endpoint, a: 1 });
  for (let index = 0; index < 14; index += 1) {
    const lightness = (failing + passing) / 2;
    const candidate = hslToRgb({ ...hsl, l: lightness, a: 1 });
    if (contrastRatio(candidate, background) >= target) {
      best = candidate;
      passing = lightness;
    } else {
      failing = lightness;
    }
  }
  return best;
}

function composite(foreground: Rgba, background: Rgba): Rgba {
  const alpha = foreground.a + background.a * (1 - foreground.a);
  if (alpha <= 0) return TRANSPARENT;
  return {
    r: (foreground.r * foreground.a + background.r * background.a * (1 - foreground.a)) / alpha,
    g: (foreground.g * foreground.a + background.g * background.a * (1 - foreground.a)) / alpha,
    b: (foreground.b * foreground.a + background.b * background.a * (1 - foreground.a)) / alpha,
    a: alpha,
  };
}

function relativeLuminance(color: Rgba) {
  return [color.r, color.g, color.b]
    .map((channel) => channel / 255)
    .map((channel) => channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4)
    .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
}

function rgbToHsl(color: Rgba): Hsla {
  const red = color.r / 255;
  const green = color.g / 255;
  const blue = color.b / 255;
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const delta = max - min;
  const lightness = (max + min) / 2;
  let hue = 0;
  if (delta) {
    if (max === red) hue = ((green - blue) / delta) % 6;
    else if (max === green) hue = (blue - red) / delta + 2;
    else hue = (red - green) / delta + 4;
    hue = (hue * 60 + 360) % 360;
  }
  const saturation = delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1));
  return { h: hue, s: saturation, l: lightness, a: color.a };
}

function hslToRgb(color: Hsla): Rgba {
  const chroma = (1 - Math.abs(2 * color.l - 1)) * color.s;
  const section = color.h / 60;
  const secondary = chroma * (1 - Math.abs((section % 2) - 1));
  const [red, green, blue] = section < 1 ? [chroma, secondary, 0]
    : section < 2 ? [secondary, chroma, 0]
      : section < 3 ? [0, chroma, secondary]
        : section < 4 ? [0, secondary, chroma]
          : section < 5 ? [secondary, 0, chroma]
            : [chroma, 0, secondary];
  const match = color.l - chroma / 2;
  return {
    r: (red + match) * 255,
    g: (green + match) * 255,
    b: (blue + match) * 255,
    a: color.a,
  };
}

function serializeColor(color: Rgba) {
  const red = Math.round(clamp(color.r, 0, 255));
  const green = Math.round(clamp(color.g, 0, 255));
  const blue = Math.round(clamp(color.b, 0, 255));
  if (color.a >= 0.999) return `rgb(${red}, ${green}, ${blue})`;
  return `rgba(${red}, ${green}, ${blue}, ${round(color.a, 3)})`;
}

function splitSelectorList(selector: string) {
  const values: string[] = [];
  let start = 0;
  let brackets = 0;
  let parentheses = 0;
  let quote = "";
  let escaped = false;
  for (let index = 0; index < selector.length; index += 1) {
    const character = selector[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === "\\") {
      escaped = true;
      continue;
    }
    if (quote) {
      if (character === quote) quote = "";
      continue;
    }
    if (character === "\"" || character === "'") quote = character;
    else if (character === "[") brackets += 1;
    else if (character === "]") brackets = Math.max(0, brackets - 1);
    else if (character === "(") parentheses += 1;
    else if (character === ")") parentheses = Math.max(0, parentheses - 1);
    else if (character === "," && brackets === 0 && parentheses === 0) {
      values.push(selector.slice(start, index).trim());
      start = index + 1;
    }
  }
  values.push(selector.slice(start).trim());
  return values.filter(Boolean);
}

function selectorSpecificity(selector: string) {
  let value = selector;
  const attributes = value.match(/\[[^\]]*\]/g)?.length ?? 0;
  value = value.replace(/\[[^\]]*\]/g, "");
  const ids = value.match(/#[\w-]+/g)?.length ?? 0;
  const classes = value.match(/\.[\w-]+/g)?.length ?? 0;
  const pseudoElements = value.match(/::[\w-]+/g)?.length ?? 0;
  const pseudoClasses = value.match(/(^|[^:]):[\w-]+(?:\([^)]*\))?/g)?.length ?? 0;
  const stripped = value
    .replace(/#[\w-]+|\.[\w-]+|::?[\w-]+(?:\([^)]*\))?/g, " ");
  const types = stripped
    .split(/[\s>+~]+/)
    .filter((part) => /^[a-zA-Z][\w-]*$/.test(part)).length;
  return ids * 1_000_000 + (classes + attributes + pseudoClasses) * 1_000 + types + pseudoElements;
}

function matches<T extends string>(value: string, candidates: readonly T[]): value is T {
  return candidates.some((candidate) => candidate === value);
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function round(value: number, digits: number) {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}
