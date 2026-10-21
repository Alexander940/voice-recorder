#!/usr/bin/env node
// HU-16: el arnés del oráculo visual (src/harness) nunca entra en el bundle de producción.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

const DIST = "dist";
const MARKERS = ["setupHarness", "harnessReady", "mockIPC", "Pantalla desconocida"];

function files(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? files(p) : [p];
  });
}

const found = files(DIST)
  .filter((f) => /\.(js|html|css)$/.test(f))
  .flatMap((f) => {
    const src = readFileSync(f, "utf8");
    return MARKERS.filter((m) => src.includes(m)).map((m) => `${f}: ${m}`);
  });

if (found.length) {
  console.error("El arnés está en el bundle de producción:\n  " + found.join("\n  "));
  process.exit(1);
}
console.log("OK: el bundle de producción no contiene el arnés.");
