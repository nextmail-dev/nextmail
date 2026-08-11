import { describe, expect, it } from "vitest";

import mixedBackgroundTable from "../../../testdata/mail-rendering/mixed-background-table.html?raw";

import {
  contrastRatio,
  hasAuthoredDarkMode,
  invertLightness,
  smartInvertMailDocument,
} from "./mail-dark-mode";

describe("mail smart inversion", () => {
  it("inverts lightness while retaining the dominant hue", () => {
    const white = invertLightness({ r: 255, g: 255, b: 255, a: 1 });
    const black = invertLightness({ r: 0, g: 0, b: 0, a: 1 });
    const blue = invertLightness({ r: 32, g: 96, b: 224, a: 1 });

    expect(white.r).toBeCloseTo(23, 0);
    expect(black.r).toBeCloseTo(230, 0);
    expect(blue.b).toBeGreaterThan(blue.g);
    expect(blue.g).toBeGreaterThan(blue.r);
  });

  it("computes the sanitized cascade and writes accessible inline colors", () => {
    const result = smartInvertMailDocument(`<!doctype html><html><head><style>
      .card { color: #fefefe; background-color: #ffffff; border: 1px solid #dddddd; }
      #card { color: #202124; }
      .card .badge { color: rgb(180, 30, 60); background-color: rgb(245, 235, 238); }
    </style></head><body><div class="card" id="card"><span class="badge">Status</span></div></body></html>`);
    const document = new DOMParser().parseFromString(result, "text/html");
    const card = document.querySelector<HTMLElement>("#card")!;
    const badge = document.querySelector<HTMLElement>(".badge")!;

    expect(card.style.getPropertyPriority("color")).toBe("important");
    expect(card.style.backgroundColor).not.toBe("rgb(255, 255, 255)");
    expect(card.style.borderTopColor).not.toBe("rgb(221, 221, 221)");
    expect(badge.style.getPropertyPriority("color")).toBe("important");
    expect(contrastRatio(readRgb(badge.style.color), readRgb(badge.style.backgroundColor)))
      .toBeGreaterThanOrEqual(4.5);
  });

  it("honors important rules, inline specificity, inheritance, and presentational colors", () => {
    const result = smartInvertMailDocument(`<!doctype html><html><head><style>
      .message { color: #101010 !important; }
      #message { color: #fefefe; }
      td { background-color: #ffffff; }
    </style></head><body><div class="message" id="message" style="color:#777777">
      <table><tbody><tr><td bgcolor="#f8f8f8"><font color="#2244aa">Text</font></td></tr></tbody></table>
    </div></body></html>`);
    const document = new DOMParser().parseFromString(result, "text/html");
    const message = document.querySelector<HTMLElement>("#message")!;
    const cell = document.querySelector<HTMLElement>("td")!;
    const font = document.querySelector<HTMLElement>("font")!;

    expect(message.style.color).not.toBe("rgb(254, 254, 254)");
    expect(cell.style.backgroundColor).not.toBe("rgb(248, 248, 248)");
    expect(font.style.color).not.toBe("rgb(34, 68, 170)");
  });

  it("adapts legacy hr and table border color attributes", () => {
    const result = smartInvertMailDocument(`<!doctype html><html><body>
      <hr id="divider" color="#b5c4df">
      <table id="table" border="1" bordercolor="#000000"><tbody><tr><td>Cell</td></tr></tbody></table>
    </body></html>`);
    const document = new DOMParser().parseFromString(result, "text/html");
    const divider = document.querySelector<HTMLElement>("#divider")!;
    const table = document.querySelector<HTMLElement>("#table")!;

    expect(contrastRatio(readRgb(divider.style.borderTopColor), { r: 23, g: 23, b: 23, a: 1 }))
      .toBeGreaterThanOrEqual(3);
    expect(table.style.borderTopColor).not.toBe("rgb(23, 23, 23)");
    expect(readRgb(table.style.borderTopColor).r).toBeGreaterThan(150);
  });

  it("adapts every authored light and dark cell in the shared rendering corpus", () => {
    const result = smartInvertMailDocument(mixedBackgroundTable);
    const document = new DOMParser().parseFromString(result, "text/html");
    const cells = Array.from(document.querySelectorAll<HTMLElement>("td"));

    expect(cells[0].style.backgroundColor).not.toBe("rgb(255, 255, 255)");
    expect(cells[1].style.backgroundColor).not.toBe("rgb(32, 33, 36)");
    for (const cell of cells.slice(0, 2)) {
      expect(contrastRatio(readRgb(cell.style.color), readRgb(cell.style.backgroundColor)))
        .toBeGreaterThanOrEqual(4.5);
    }
  });

  it("darkens chromatic highlights before changing their authored text color", () => {
    const result = smartInvertMailDocument(`<!doctype html><html><body>
      <span id="highlight" style="background-color:#ffff00"><strong id="label" style="color:#ff0000">Highlighted</strong></span>
    </body></html>`);
    const document = new DOMParser().parseFromString(result, "text/html");
    const highlight = document.querySelector<HTMLElement>("#highlight")!;
    const label = document.querySelector<HTMLElement>("#label")!;
    const foreground = readRgb(label.style.color);
    const background = readRgb(highlight.style.backgroundColor);

    expect(background.r).toBeLessThan(140);
    expect(background.g).toBeLessThan(140);
    expect(background.b).toBeLessThan(30);
    expect(foreground.r).toBeGreaterThan(200);
    expect(foreground.g).toBeLessThan(80);
    expect(foreground.b).toBeLessThan(80);
    expect(contrastRatio(foreground, background)).toBeGreaterThanOrEqual(4.5);
  });

  it("leaves image and video colors unchanged, including transparent PNG images", () => {
    const result = smartInvertMailDocument(`<!doctype html><html><body>
      <img id="logo" src="data:image/png;base64,aW1hZ2U=" style="color:#111;background-color:#fff">
      <img id="photo" src="https://images.example/photo.jpg" style="background-color:#fff">
      <video id="video" style="color:#111;background-color:#fff"></video>
    </body></html>`);
    const document = new DOMParser().parseFromString(result, "text/html");
    const logo = document.querySelector<HTMLElement>("#logo")!;
    const photo = document.querySelector<HTMLElement>("#photo")!;
    const video = document.querySelector<HTMLElement>("#video")!;

    expect(logo.style.backgroundColor).toBe("rgb(255, 255, 255)");
    expect(photo.style.backgroundColor).toBe("rgb(255, 255, 255)");
    expect(video.style.backgroundColor).toBe("rgb(255, 255, 255)");
  });

  it("recognizes only the internal post-sanitize native-dark marker", () => {
    expect(hasAuthoredDarkMode('<html data-nextmail-native-dark="">')).toBe(true);
    expect(hasAuthoredDarkMode('<meta name="color-scheme" content="light dark">')).toBe(false);
  });
});

function readRgb(value: string) {
  const values = value.match(/[\d.]+/g)?.map(Number) ?? [];
  return { r: values[0], g: values[1], b: values[2], a: values[3] ?? 1 };
}
