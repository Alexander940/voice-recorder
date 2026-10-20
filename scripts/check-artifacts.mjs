// Oracle de HU-14: verifica que el build local de Windows generó los instaladores.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const { productName, version } = conf;
const bundle = "src-tauri/target/release/bundle";
const expected = [
  ["msi", `${productName}_${version}_x64_en-US.msi`],
  ["nsis", `${productName}_${version}_x64-setup.exe`],
];

const errors = [];
for (const [dir, name] of expected) {
  const path = join(bundle, dir, name);
  if (!existsSync(path)) {
    const found = existsSync(join(bundle, dir)) ? readdirSync(join(bundle, dir)) : [];
    errors.push(`falta ${path} (hay: ${found.join(", ") || "nada"})`);
  } else if (statSync(path).size < 500_000) {
    errors.push(`${path} es sospechosamente pequeño`);
  } else {
    console.log(`✓ ${path} (${(statSync(path).size / 1_048_576).toFixed(2)} MiB)`);
  }
}

if (errors.length) {
  console.error(errors.map((e) => `✗ ${e}`).join("\n"));
  process.exit(1);
}
