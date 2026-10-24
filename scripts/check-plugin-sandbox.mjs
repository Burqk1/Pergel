// Checks the Figma plugin sandbox (src/lib/figmaSandbox.ts) in Node with a fake document: a
// Figma-style plugin works, plugin code can't reach the app, and runaway code is stopped.
// Run with `npm run check:sandbox`.
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const project = join(dirname(fileURLToPath(import.meta.url)), "..");
const here = mkdtempSync(join(tmpdir(), "pergel-sandbox-"));
const require = createRequire(join(project, "package.json"));
const ts = require("typescript");

const prelude = readFileSync(join(project, "src/lib/figmaPrelude.js"), "utf8");
let src = readFileSync(join(project, "src/lib/figmaSandbox.ts"), "utf8");
src = src.replace(`import prelude from "./figmaPrelude.js?raw";`, `const prelude = ${JSON.stringify(prelude)};`);
// Resolve the packages from the project.
src = src
  .replace(`import("quickjs-emscripten-core")`, `import(${JSON.stringify(pathToFileURL(require.resolve("quickjs-emscripten-core")).href.replace(/index\.js$/, "index.mjs"))})`)
  .replace(
    `import("@jitl/quickjs-singlefile-browser-release-sync")`,
    `import(${JSON.stringify(pathToFileURL(join(project, "node_modules/@jitl/quickjs-singlefile-browser-release-sync/dist/index.mjs")).href)})`,
  );
const js = ts.transpileModule(src, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const out = join(here, "figmaSandbox.mjs");
writeFileSync(out, js);
globalThis.window = globalThis;
const { Sandbox } = await import(pathToFileURL(out).href);

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures++;
};

// A fake document: layers by id.
const nodes = new Map();
let next = 1;
const calls = [];
const host = (op, args) => {
  calls.push(op);
  if (op === "agent") {
    const { tool, args: a } = args;
    if (tool === "create_nodes") {
      const id = String(next++);
      nodes.set(id, { name: a.nodes[0].type, x: 0, fill: a.nodes[0].fill });
      return { created: [{ id }] };
    }
    if (tool === "update_node") {
      Object.assign(nodes.get(a.nodeId), a);
      return { updated: {} };
    }
    if (tool === "get_node_details") {
      const n = nodes.get(a.nodeId);
      return { type: "rectangle", name: n.name, relX: n.x, fillOn: true, fill: "#FF0000", fillOpacity: 100 };
    }
    if (tool === "list_children") return { children: a.nodeId ? [] : [...nodes.keys()] };
    throw new Error(`no ${tool}`);
  }
  if (op === "selection") return [];
  if (op === "setSelection" || op === "log" || op === "notify" || op === "close" || op === "setTimeout") return null;
  if (op === "pageName") return "Page 1";
  throw new Error(`unexpected ${op}`);
};

// Figma's "Rectangle Creator" sample, main code.
const box = await Sandbox.create(host);
await box.run(
  `const nodes = [];
  for (let i = 0; i < 3; i++) {
    const rect = figma.createRectangle();
    rect.x = i * 150;
    rect.fills = [{ type: "SOLID", color: { r: 1, g: 0.5, b: 0 } }];
    figma.currentPage.appendChild ? null : null;
    nodes.push(rect);
  }
  figma.currentPage.selection = nodes;
  figma.notify("made " + nodes.length);
  await figma.clientStorage.setAsync ? 0 : 0;
  figma.closePlugin();`,
  "",
);
check(nodes.size === 3, "plugin created 3 rectangles");
check(nodes.get("3").x === 300 && nodes.get("3").fill === "#FF8000", "property writes reach the document");
check(box.eval(`figma.currentPage.findAll().length`) === 3, "findAll reads the document");
check(box.eval(`figma.getNodeById ? figma.currentPage.children[0] === figma.currentPage.children[0] : false`) === true, "same layer, same object");

// Escapes: the sandbox has no window, DOM or app; the API's functions lead back into the sandbox.
check(box.eval(`typeof window + typeof document + typeof fetch + typeof __TAURI_INTERNALS__ + typeof require + typeof process`) === "undefinedundefinedundefinedundefinedundefinedundefined", "no window, document, fetch, Tauri, require or process");
check(box.eval(`figma.createRectangle.constructor("return typeof globalThis.__host")()`) === "function", "Function from the API runs in the sandbox, not the app");
check(box.eval(`(function () { return this })() === globalThis`) === true, "the global object is the sandbox's own");
check(box.eval(`typeof figma.currentPage.selection.constructor.constructor("return globalThis.process")()`) === "undefined", "no way to the app's process through prototypes");
check(globalThis.__host === undefined && globalThis.figma === undefined, "nothing leaked into the app's global object");

// Runaway code is stopped.
const started = Date.now();
let stopped = false;
try {
  box.eval(`while (true) {}`);
} catch (e) {
  stopped = /interrupted/i.test(String(e?.message ?? e));
}
check(stopped && Date.now() - started < 15000, "an endless loop is interrupted");
let capped = false;
try {
  box.eval(`const a = []; while (true) a.push(new Array(1e6).fill(1));`);
} catch (e) {
  capped = true;
}
check(capped, "memory is capped");
box.dispose();
console.log(failures ? `${failures} failed` : "all passed");
process.exit(failures ? 1 : 0);
