import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const release = !process.argv.includes("--dev");
const emsdk = process.env.EMSDK || join(root, "..", "tools", "emsdk");
if (!existsSync(join(emsdk, "upstream", "emscripten"))) {
  console.error(`Emscripten not found at ${emsdk}. Install it or set EMSDK.`);
  process.exit(1);
}

const sub = (dir) => {
  const base = join(emsdk, dir);
  const first = existsSync(base) ? readdirSync(base)[0] : null;
  return first ? join(base, first) : null;
};
const node = sub("node");
const python = sub("python");
const sep = process.platform === "win32" ? ";" : ":";
const paths = [emsdk, join(emsdk, "upstream", "emscripten"), node && join(node, "bin"), python].filter(Boolean);

const env = {
  ...process.env,
  EMSDK: emsdk,
  EM_CONFIG: join(emsdk, ".emscripten"),
  EMCC_CFLAGS: "-s ERROR_ON_UNDEFINED_SYMBOLS=0",
  CC_wasm32_unknown_emscripten: join(emsdk, "upstream", "emscripten", process.platform === "win32" ? "emcc.exe" : "emcc"),
  CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER: join(emsdk, "upstream", "emscripten", process.platform === "win32" ? "em++.exe" : "em++"),
  AR_wasm32_unknown_emscripten: join(emsdk, "upstream", "emscripten", process.platform === "win32" ? "emar.exe" : "emar"),
  PATH: [...paths, process.env.PATH].join(sep),
};

const profile = release ? "release" : "debug";
const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
execFileSync(
  cargo,
  ["build", "-p", "pergel-wasm", "--target", "wasm32-unknown-emscripten", ...(release ? ["--release"] : [])],
  { cwd: root, env, stdio: "inherit" },
);

const outDir = join(root, "target", "wasm32-unknown-emscripten", profile);
const dest = join(root, "public", "engine");
mkdirSync(dest, { recursive: true });
for (const name of readdirSync(outDir).filter((f) => /^pergel[-_]wasm\.(js|wasm)$/.test(f))) {
  copyFileSync(join(outDir, name), join(dest, name));
  console.log(`engine -> public/engine/${name}`);
}
