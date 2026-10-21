#!/usr/bin/env node
// Oráculo visual de HU-16: compara cada pantalla implementada con su mockup aprobado.
//
// Congelado tras la calibración (ver docs/loops-history/hu-16-visual-redesign/design.md):
// cambiar este archivo, los umbrales, los mockups, tray-spec.json o deviations.json
// requiere una enmienda aprobada por el usuario.
//
// Uso: node scripts/visual-oracle.mjs [--only <id,id>] [--self]
//   --self  compara cada mockup consigo mismo (calibración del oráculo).

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";
import { createServer } from "vite";
import { chromium } from "playwright";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";
import { AxeBuilder } from "@axe-core/playwright";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DESIGN = join(ROOT, "docs/design");
const MOCKUPS = join(DESIGN, "mockups");
const OUT = join(ROOT, "target/visual-report");
const STATIC = join(ROOT, "target/visual-oracle/mockups");

export const THRESHOLDS = {
  centerPx: 12,
  fontSizePx: 1,
  deltaE: 3,
  pixelThreshold: 0.1,
  maxMismatch: 0.03,
  axeImpacts: ["serious", "critical"],
};

const TITLEBAR = 36;

// Pantallas comparadas. `unit` recorta un elemento (mock e impl) en vez de la ventana entera.
// Los selectores `data-*` de la implementación son parte del contrato.
export const SCREENS = [
  { id: "onboarding-welcome", mock: "Main.dc.html", size: [640, 520] },
  { id: "onboarding-groq", mock: "Onboarding-Groq.dc.html", size: [640, 520] },
  { id: "onboarding-permissions", mock: "Onboarding-Permisos.dc.html", size: [640, 520] },
  { id: "onboarding-test", mock: "Onboarding-Prueba.dc.html", size: [640, 520] },
  { id: "history", mock: "Historial.dc.html", size: [960, 640] },
  { id: "modes", mock: "Modos.dc.html", size: [960, 640] },
  { id: "vocabulary", mock: "Vocabulario.dc.html", size: [960, 640] },
  { id: "settings", mock: "Ajustes.dc.html", size: [960, 1040] },
  { id: "accounts", mock: "Cuentas.dc.html", size: [960, 640] },
  { id: "updates", mock: "Actualizaciones.dc.html", size: [960, 640] },
  {
    id: "overlay-recording",
    mock: "Overlay.dc.html",
    size: [760, 440],
    harness: "overlay-recording",
    unit: { mock: '[aria-label="Grabando"]', impl: "[data-overlay-pill]" },
  },
  {
    id: "overlay-recording-hint",
    mock: "Overlay.dc.html",
    size: [760, 440],
    harness: "overlay-recording",
    unit: { mock: '[aria-label="Grabando"] + span', impl: "[data-overlay-hint]" },
  },
  {
    id: "overlay-processing",
    mock: "Overlay.dc.html",
    size: [760, 440],
    unit: { mock: '[aria-label="Procesando"]', impl: "[data-overlay-pill]" },
  },
  {
    id: "overlay-pasted",
    mock: "Overlay.dc.html",
    size: [760, 440],
    unit: { mock: '[aria-label="Pegado"]', impl: "[data-overlay-pill]" },
  },
  {
    id: "overlay-error",
    mock: "Overlay.dc.html",
    size: [760, 440],
    unit: { mock: '[role="alert"]', impl: "[data-overlay-pill]" },
  },
];

const NO_ANIMATION = `*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}`;

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/** Comprueba que los archivos congelados no han cambiado. */
export function verifyFrozen() {
  const sums = readFileSync(join(MOCKUPS, "SHA256SUMS"), "utf8").trim().split("\n");
  const problems = [];
  for (const line of sums) {
    const [hash, star] = line.split(/\s+/);
    const rel = star.replace(/^\*/, "");
    const path = join(DESIGN, rel);
    if (!existsSync(path)) problems.push(`falta ${rel}`);
    else if (sha256(path) !== hash) problems.push(`${rel} ha cambiado`);
  }
  const design = readFileSync(
    join(ROOT, "docs/loops-history/hu-16-visual-redesign/design.md"),
    "utf8",
  );
  const pinned = design.match(/oracle_sha256:\s*`?([0-9a-f]{64})`?/);
  if (pinned && pinned[1] !== sha256(fileURLToPath(import.meta.url))) {
    problems.push("scripts/visual-oracle.mjs no coincide con el hash calibrado");
  }
  // El arnés solo simula el backend: nunca puede cargar los mockups.
  const harnessDir = join(ROOT, "src/harness");
  if (existsSync(harnessDir)) {
    for (const f of readdirSync(harnessDir)) {
      const src = readFileSync(join(harnessDir, f), "utf8");
      if (/docs\/design|\.dc\.html|visual-oracle/.test(src)) {
        problems.push(`src/harness/${f} referencia los mockups`);
      }
    }
  }
  return problems;
}

/** Mockup `.dc.html` → HTML estático con los valores por defecto de sus props y fuentes locales. */
export function staticMockup(source) {
  const props = source.match(/data-props='([^']*)'/);
  const defaults = props ? JSON.parse(props[1].replace(/&#39;/g, "'").replace(/&amp;/g, "&")) : {};
  return source
    .replace(/<script src="\.\/support\.js"><\/script>/, "")
    .replace(/<script type="text\/x-dc"[\s\S]*?<\/script>/, "")
    .replace(/<\/?(x-dc|helmet)>/g, "")
    .replace(/<link[^>]*fonts\.(googleapis|gstatic)\.com[^>]*>/g, "")
    .replace(
      "</head>",
      `<link rel="stylesheet" href="/src/styles/fonts.css"><style>${NO_ANIMATION}</style></head>`,
    )
    .replace(/\{\{\s*(\w+)\s*\}\}/g, (_, name) => String(defaults[name]?.default ?? ""));
}

// ---------- Extracción de textos (se ejecuta en el navegador) ----------

function extractTexts(root) {
  const base = root.getBoundingClientRect();
  const out = [];
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    return (
      r.width > 1 &&
      r.height > 1 &&
      el.checkVisibility({ opacityProperty: true, visibilityProperty: true })
    );
  };
  const push = (el, text, rect) => {
    const cs = getComputedStyle(el);
    out.push({
      text,
      cx: rect.left + rect.width / 2 - base.left,
      cy: rect.top + rect.height / 2 - base.top,
      fontSize: parseFloat(cs.fontSize),
      fontWeight: cs.fontWeight,
      color: cs.color,
    });
  };
  for (const el of [root, ...root.querySelectorAll("*")]) {
    if (el.closest("svg, script, style, option, head")) continue;
    if (!visible(el)) continue;
    const tag = el.tagName.toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select") {
      const type = (el.getAttribute("type") || "text").toLowerCase();
      if (["checkbox", "radio", "hidden"].includes(type)) continue;
      const text =
        tag === "select"
          ? (el.selectedOptions[0]?.textContent ?? "")
          : el.value || el.getAttribute("placeholder") || "";
      if (text.trim())
        push(el, `[${tag}] ${text.replace(/\s+/g, " ").trim()}`, el.getBoundingClientRect());
      continue;
    }
    let raw = "";
    let box = null;
    for (const node of el.childNodes) {
      if (node.nodeType !== Node.TEXT_NODE) continue;
      raw += node.textContent;
      if (!node.textContent.trim()) continue;
      const range = document.createRange();
      range.selectNodeContents(node);
      const r = range.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue;
      box = box
        ? {
            left: Math.min(box.left, r.left),
            top: Math.min(box.top, r.top),
            right: Math.max(box.right, r.right),
            bottom: Math.max(box.bottom, r.bottom),
          }
        : { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    }
    const text = raw.replace(/\s+/g, " ").trim();
    if (!text || !box) continue;
    push(el, text, {
      left: box.left,
      top: box.top,
      width: box.right - box.left,
      height: box.bottom - box.top,
    });
  }
  return out;
}

// ---------- Comparación ----------

function parseRgb(css) {
  const m = css.match(/rgba?\(([^)]+)\)/);
  if (!m) return [0, 0, 0];
  return m[1]
    .split(/[ ,/]+/)
    .filter(Boolean)
    .slice(0, 3)
    .map(Number);
}

function toLab([r, g, b]) {
  const lin = (c) => {
    c /= 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const [R, G, B] = [lin(r), lin(g), lin(b)];
  const x = (R * 0.4124 + G * 0.3576 + B * 0.1805) / 0.95047;
  const y = R * 0.2126 + G * 0.7152 + B * 0.0722;
  const z = (R * 0.0193 + G * 0.1192 + B * 0.9505) / 1.08883;
  const f = (t) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  return [116 * f(y) - 16, 500 * (f(x) - f(y)), 200 * (f(y) - f(z))];
}

export function deltaE(a, b) {
  const [l1, a1, b1] = toLab(parseRgb(a));
  const [l2, a2, b2] = toLab(parseRgb(b));
  return Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

export function compareTexts(mock, impl, ignore = []) {
  const failures = [];
  const used = new Set();
  for (const m of mock) {
    if (ignore.includes(m.text)) continue;
    const candidates = impl
      .map((t, i) => ({ t, i }))
      .filter(({ t, i }) => t.text === m.text && !used.has(i))
      .sort(
        (p, q) =>
          Math.hypot(p.t.cx - m.cx, p.t.cy - m.cy) - Math.hypot(q.t.cx - m.cx, q.t.cy - m.cy),
      );
    if (candidates.length === 0) {
      failures.push({ text: m.text, problem: "falta" });
      continue;
    }
    const { t, i } = candidates[0];
    used.add(i);
    const dist = Math.hypot(t.cx - m.cx, t.cy - m.cy);
    const problems = [];
    if (dist > THRESHOLDS.centerPx) problems.push(`desplazado ${dist.toFixed(1)} px`);
    if (Math.abs(t.fontSize - m.fontSize) > THRESHOLDS.fontSizePx)
      problems.push(`tamaño ${t.fontSize} ≠ ${m.fontSize}`);
    if (t.fontWeight !== m.fontWeight) problems.push(`peso ${t.fontWeight} ≠ ${m.fontWeight}`);
    const dE = deltaE(t.color, m.color);
    if (dE > THRESHOLDS.deltaE)
      problems.push(`color ${t.color} ≠ ${m.color} (ΔE ${dE.toFixed(1)})`);
    if (problems.length) failures.push({ text: m.text, problem: problems.join("; ") });
  }
  return failures;
}

function pad(png, w, h, fill) {
  if (png.width === w && png.height === h) return png;
  const out = new PNG({ width: w, height: h });
  for (let i = 0; i < w * h; i++) out.data.set(fill, i * 4);
  PNG.bitblt(png, out, 0, 0, Math.min(png.width, w), Math.min(png.height, h), 0, 0);
  return out;
}

export function comparePixels(mockBuf, implBuf) {
  const a = PNG.sync.read(mockBuf);
  const b = PNG.sync.read(implBuf);
  const w = Math.max(a.width, b.width);
  const h = Math.max(a.height, b.height);
  // Relleno distinto en cada lado: una diferencia de tamaño cuenta como diferencia.
  const pa = pad(a, w, h, [255, 0, 255, 255]);
  const pb = pad(b, w, h, [0, 255, 0, 255]);
  const diff = new PNG({ width: w, height: h });
  const n = pixelmatch(pa.data, pb.data, diff.data, w, h, {
    threshold: THRESHOLDS.pixelThreshold,
    includeAA: false,
  });
  return {
    ratio: n / (w * h),
    diff: PNG.sync.write(diff),
    sizes: { mock: [a.width, a.height], impl: [b.width, b.height] },
  };
}

// ---------- Captura ----------

async function capture(page, unitSelector, clip) {
  if (unitSelector) {
    const el = page.locator(unitSelector).first();
    if ((await el.count()) === 0) return null;
    const png = await el.screenshot({ animations: "disabled" });
    const texts = await el.evaluate(extractTexts);
    return { png, texts };
  }
  const png = await page.screenshot({ clip, animations: "disabled" });
  const texts = await page.evaluate(
    ([extract, top]) => {
      // Se recorta la barra de título simulada: los textos se miden desde `top`.
      const fn = new Function(`return (${extract})`)();
      const res = fn(document.body);
      return res.filter((t) => t.cy >= top).map((t) => ({ ...t, cy: t.cy - top }));
    },
    [extractTexts.toString(), clip.y],
  );
  return { png, texts };
}

async function openMock(browser, base, screen) {
  const [w, h] = screen.size;
  const page = await browser.newPage({ viewport: { width: w, height: h }, deviceScaleFactor: 1 });
  await page.goto(
    `${base}/target/visual-oracle/mockups/${screen.mock.replace(".dc.html", ".html")}`,
  );
  await page.evaluate(() => document.fonts.ready);
  const fonts = await page.evaluate(async () => {
    const a = await document.fonts.load("400 16px Geist");
    const b = await document.fonts.load("500 12px 'Geist Mono'");
    return a.length > 0 && b.length > 0;
  });
  if (!fonts) throw new Error(`Geist no se cargó en ${screen.mock}: el oráculo no es fiable`);
  return page;
}

async function openImpl(browser, base, screen) {
  const [w, h] = screen.size;
  const viewport = screen.unit ? { width: w, height: 200 } : { width: w, height: h - TITLEBAR };
  // axe necesita un contexto propio (no `browser.newPage`).
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 1,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  await page.goto(`${base}/?harness=${screen.harness ?? screen.id}`);
  await page.addStyleTag({ content: NO_ANIMATION });
  await page
    .waitForFunction(() => document.documentElement.dataset.harnessReady === "1", null, {
      timeout: 10_000,
    })
    .catch(() => {});
  await page.evaluate(() => document.fonts.ready);
  return page;
}

async function main() {
  const args = process.argv.slice(2);
  const self = args.includes("--self");
  const onlyIdx = args.indexOf("--only");
  const only = onlyIdx >= 0 ? args[onlyIdx + 1].split(",") : null;

  const frozen = verifyFrozen();
  if (frozen.length) {
    console.error("Oráculo abortado, archivos congelados alterados:\n  " + frozen.join("\n  "));
    process.exit(2);
  }

  rmSync(OUT, { recursive: true, force: true });
  mkdirSync(join(OUT, "img"), { recursive: true });
  mkdirSync(STATIC, { recursive: true });
  for (const f of readdirSync(MOCKUPS).filter((f) => f.endsWith(".dc.html"))) {
    writeFileSync(
      join(STATIC, f.replace(".dc.html", ".html")),
      staticMockup(readFileSync(join(MOCKUPS, f), "utf8")),
    );
  }

  const deviations = JSON.parse(readFileSync(join(DESIGN, "deviations.json"), "utf8"));
  const server = await createServer({
    root: ROOT,
    configFile: join(ROOT, "vite.config.ts"),
    logLevel: "error",
    server: { port: 5199, strictPort: false, hmr: false },
  });
  await server.listen();
  const base = server.resolvedUrls.local[0].replace(/\/$/, "");
  const browser = await chromium.launch();

  const results = [];
  try {
    for (const screen of SCREENS.filter((s) => !only || only.includes(s.id))) {
      const [w, h] = screen.size;
      const clip = { x: 0, y: TITLEBAR, width: w, height: h - TITLEBAR };
      const mockPage = await openMock(browser, base, screen);
      const mock = await capture(mockPage, screen.unit?.mock, clip);
      let implPage;
      let impl;
      if (self) {
        // Segundo renderizado independiente del mismo mockup: mide el determinismo.
        implPage = await openMock(browser, base, screen);
        impl = await capture(implPage, screen.unit?.mock, clip);
      } else {
        implPage = await openImpl(browser, base, screen);
        impl = await capture(implPage, screen.unit?.impl, {
          x: 0,
          y: 0,
          width: w,
          height: h - TITLEBAR,
        });
      }
      const r = { id: screen.id, mock: screen.mock };
      if (!mock) {
        r.error = "el selector del mockup no encuentra nada (oráculo roto)";
      } else if (!impl) {
        r.error = `la implementación no tiene ${screen.unit?.impl}`;
      } else {
        const ignore = deviations.ignore_text?.[screen.mock.replace(".dc.html", "")] ?? [];
        r.textFailures = compareTexts(mock.texts, impl.texts, ignore);
        r.textsChecked = mock.texts.length;
        const px = comparePixels(mock.png, impl.png);
        r.mismatch = px.ratio;
        r.sizes = px.sizes;
        writeFileSync(join(OUT, "img", `${screen.id}-mock.png`), mock.png);
        writeFileSync(join(OUT, "img", `${screen.id}-impl.png`), impl.png);
        writeFileSync(join(OUT, "img", `${screen.id}-diff.png`), px.diff);
      }
      if (!self && impl) {
        const axe = await new AxeBuilder({ page: implPage }).analyze();
        r.axe = axe.violations
          .filter((v) => THRESHOLDS.axeImpacts.includes(v.impact))
          .map((v) => `${v.id} (${v.impact}): ${v.nodes.length} nodo(s)`);
      } else {
        r.axe = [];
      }
      r.pass =
        !r.error &&
        r.textFailures.length === 0 &&
        r.mismatch <= THRESHOLDS.maxMismatch &&
        r.axe.length === 0;
      results.push(r);
      await mockPage.close();
      await implPage.close();
      const mark = r.pass ? "PASS" : "FAIL";
      const px = r.mismatch === undefined ? "-" : `${(r.mismatch * 100).toFixed(2)} %`;
      const tx = r.textFailures ? `${r.textFailures.length}/${r.textsChecked}` : "-";
      console.log(
        `${mark}  ${screen.id.padEnd(24)} píxeles ${px.padStart(8)}  textos mal ${tx.padStart(7)}  axe ${r.axe.length}${r.error ? "  " + r.error : ""}`,
      );
    }
  } finally {
    await browser.close();
    await server.close();
  }

  writeFileSync(
    join(OUT, "results.json"),
    JSON.stringify({ thresholds: THRESHOLDS, self, results }, null, 2),
  );
  writeFileSync(join(OUT, "index.html"), report(results, self));
  const failed = results.filter((r) => !r.pass);
  console.log(
    `\n${results.length - failed.length}/${results.length} pantallas pasan. Informe: target/visual-report/index.html`,
  );
  process.exit(failed.length ? 1 : 0);
}

function esc(s) {
  return String(s).replace(
    /[&<>"]/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c],
  );
}

function report(results, self) {
  const rows = results
    .map((r) => {
      const imgs = r.error
        ? `<p class="err">${esc(r.error)}</p>`
        : ["mock", "impl", "diff"]
            .map(
              (k) =>
                `<figure><figcaption>${{ mock: "Mockup", impl: "Implementación", diff: "Diferencias" }[k]}</figcaption><img src="img/${r.id}-${k}.png" alt="${k} ${r.id}"></figure>`,
            )
            .join("");
      const texts = (r.textFailures ?? [])
        .map((f) => `<li><code>${esc(f.text)}</code>: ${esc(f.problem)}</li>`)
        .join("");
      const axe = r.axe.map((a) => `<li>${esc(a)}</li>`).join("");
      return `<section class="${r.pass ? "pass" : "fail"}"><h2>${r.pass ? "✔" : "✘"} ${esc(r.id)} <small>${esc(r.mock)}</small></h2>
<p>Píxeles distintos: <b>${r.mismatch === undefined ? "-" : (r.mismatch * 100).toFixed(2) + " %"}</b> (máx. ${THRESHOLDS.maxMismatch * 100} %) · Textos con problemas: <b>${r.textFailures?.length ?? "-"}</b> de ${r.textsChecked ?? "-"} · axe: <b>${r.axe.length}</b></p>
<div class="imgs">${imgs}</div>${texts ? `<details><summary>Textos</summary><ul>${texts}</ul></details>` : ""}${axe ? `<details open><summary>Accesibilidad</summary><ul>${axe}</ul></details>` : ""}</section>`;
    })
    .join("\n");
  const passed = results.filter((r) => r.pass).length;
  return `<!doctype html><html lang="es"><head><meta charset="utf-8"><title>Informe visual HU-16</title>
<style>body{font:14px system-ui,sans-serif;margin:24px;color:#15171C;background:#fff}section{border:1px solid #E6E8EC;border-radius:10px;padding:12px 16px;margin:16px 0}.pass h2{color:#166534}.fail h2{color:#B91C1C}.imgs{display:flex;gap:12px;overflow-x:auto}figure{margin:0}img{max-width:420px;border:1px solid #E6E8EC}figcaption{font-size:12px;color:#5B616E}.err{color:#B91C1C}small{color:#5B616E;font-weight:400}</style></head>
<body><h1>Informe visual HU-16${self ? " (calibración: mockup contra sí mismo)" : ""}</h1><p>${passed}/${results.length} pantallas pasan.</p>${rows}</body></html>`;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => {
    console.error(e);
    process.exit(3);
  });
}
