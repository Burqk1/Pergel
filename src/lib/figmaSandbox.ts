import type { QuickJSContext, QuickJSHandle, QuickJSRuntime, QuickJSWASMModule } from "quickjs-emscripten-core";
import prelude from "./figmaPrelude.js?raw";

// The sandbox Figma plugins run in: a separate JavaScript engine (QuickJS, compiled to
// WebAssembly) with no window, DOM, network or app access. The plugin sees only the `figma` API
// of figmaPrelude.js, which reaches the app through one function taking and returning JSON text,
// so no object of the app (and no way back to it, such as a `constructor` chain) is ever exposed.
// Memory is capped, and code that runs too long without yielding is stopped.

/** The app side: one operation with JSON arguments; its JSON result, or it throws. */
export type Host = (op: string, args: unknown) => unknown;

const MEMORY = 64 << 20;
/** The longest a plugin may run without yielding back to the app. */
const SLICE_MS = 10_000;

let module: Promise<QuickJSWASMModule> | null = null;

function load() {
  module ??= Promise.all([import("quickjs-emscripten-core"), import("@jitl/quickjs-singlefile-browser-release-sync")]).then(([core, variant]) =>
    core.newQuickJSWASMModuleFromVariant(variant.default),
  );
  return module;
}

export class Sandbox {
  private pump = 0;
  private disposed = false;

  private constructor(
    private runtime: QuickJSRuntime,
    private vm: QuickJSContext,
  ) {}

  static async create(host: Host): Promise<Sandbox> {
    const runtime = (await load()).newRuntime();
    runtime.setMemoryLimit(MEMORY);
    runtime.setMaxStackSize(1 << 20);
    const vm = runtime.newContext();
    const fn = vm.newFunction("__host", (op, args) => {
      let reply: string;
      try {
        const value = host(vm.getString(op), JSON.parse(vm.getString(args)));
        reply = JSON.stringify({ ok: true, value: value === undefined ? null : value });
      } catch (e) {
        reply = JSON.stringify({ ok: false, error: e instanceof Error ? e.message : String(e) });
      }
      return vm.newString(reply);
    });
    vm.setProp(vm.global, "__host", fn);
    fn.dispose();
    const box = new Sandbox(runtime, vm);
    box.eval(prelude, "figma-api.js");
    return box;
  }

  /** Runs code; throws what it threw (or the time limit). */
  eval(code: string, file = "plugin.js") {
    if (this.disposed) return;
    this.runtime.setInterruptHandler(deadline());
    const r = this.vm.evalCode(code, file);
    const out = this.vm.unwrapResult(r);
    const value = this.vm.dump(out);
    out.dispose();
    this.jobs();
    return value;
  }

  /** Runs the plugin's main code, as the body of an async function given `figma`, `__html__`
   *  and `__uiFiles__` (so top-level `await` works, as in Figma). Resolves when it settles. */
  run(code: string, html: string): Promise<void> {
    const setup = `globalThis.__html__ = ${JSON.stringify(html)}; globalThis.__uiFiles__ = { main: __html__ };`;
    this.eval(setup);
    const body = `globalThis.__done = false; globalThis.__failed = null;
      (async function (figma, __html__, __uiFiles__) {\n${code}\n})(figma, __html__, __uiFiles__)
        .then(() => { globalThis.__done = true; }, (e) => { globalThis.__failed = String(e && e.stack || e); globalThis.__done = true; });
      undefined;`;
    this.eval(body);
    // Promises inside the sandbox settle as their jobs run; check on every tick.
    return new Promise((resolve, reject) => {
      const check = () => {
        if (this.disposed) return resolve();
        this.jobs();
        const failed = this.eval("globalThis.__failed") as string | null;
        if (failed) return reject(new Error(failed));
        if (this.eval("globalThis.__done")) return resolve();
        setTimeout(check, 16);
      };
      check();
    });
  }

  /** Calls a global function of the sandbox with string arguments. */
  callGlobal(name: string, ...args: (string | number)[]) {
    if (this.disposed) return;
    this.runtime.setInterruptHandler(deadline());
    const fn = this.vm.getProp(this.vm.global, name);
    const handles: QuickJSHandle[] = args.map((a) => (typeof a === "number" ? this.vm.newNumber(a) : this.vm.newString(a)));
    const r = this.vm.callFunction(fn, this.vm.undefined, ...handles);
    fn.dispose();
    handles.forEach((h) => h.dispose());
    const out = this.vm.unwrapResult(r);
    out.dispose();
    this.jobs();
  }

  /** Runs the sandbox's pending promise jobs; keeps doing so while it has some. */
  private jobs() {
    if (this.disposed) return;
    this.runtime.setInterruptHandler(deadline());
    const r = this.runtime.executePendingJobs();
    if (r.error) {
      const e = this.vm.dump(r.error);
      r.error.dispose();
      console.warn("[figma plugin]", e);
    }
    if (this.runtime.hasPendingJob() && !this.pump) {
      this.pump = window.setTimeout(() => {
        this.pump = 0;
        this.jobs();
      }, 0);
    }
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    clearTimeout(this.pump);
    this.vm.dispose();
    this.runtime.dispose();
  }
}

function deadline() {
  const end = Date.now() + SLICE_MS;
  return () => Date.now() > end;
}
