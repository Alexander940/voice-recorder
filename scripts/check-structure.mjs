// Oracle de HU-01: verifica que existen los módulos Rust y los scripts npm requeridos.
import { existsSync, readFileSync } from "node:fs";

const modules = ["audio", "stt", "llm", "output", "settings", "history"];
const scripts = ["lint", "typecheck", "test", "tauri"];
const errors = [];

const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
for (const m of modules) {
  if (!existsSync(`src-tauri/src/${m}/mod.rs`)) errors.push(`falta src-tauri/src/${m}/mod.rs`);
  if (!new RegExp(`^pub mod ${m};`, "m").test(lib))
    errors.push(`lib.rs no declara \`pub mod ${m};\``);
}

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
for (const s of scripts) if (!pkg.scripts?.[s]) errors.push(`falta el script npm "${s}"`);

if (!existsSync(".github/workflows/ci.yml")) errors.push("falta .github/workflows/ci.yml");

if (errors.length) {
  console.error(errors.map((e) => `✗ ${e}`).join("\n"));
  process.exit(1);
}
console.log(`✓ estructura OK (${modules.length} módulos, ${scripts.length} scripts, CI)`);
