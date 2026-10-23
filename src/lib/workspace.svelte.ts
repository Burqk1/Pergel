import { editor } from "./editor.svelte";
import { PAGE } from "./engine";
import { files, type FileChange, type FileMeta, versions } from "./files";

/** An open file shown as a tab. `slot` is the engine's document slot. */
export interface Tab {
  slot: number;
  file: FileMeta;
  /** Engine revision at the last save; anything newer is unsaved. */
  saved: number;
}

const SESSION = "pergel.session";
const AUTOSAVE_MS = 1500;
const EMPTY_DOC = JSON.stringify({ format: "pergel/document", version: 1, pages: [{ name: "Page 1", layers: [] }] });

export type HomeView = "recents" | "drafts" | "starred" | "trash";

class Workspace {
  files = $state.raw<FileMeta[]>([]);
  tabs = $state<Tab[]>([]);
  /** The tab on screen, or "home" for the file browser. */
  active = $state<number | "home">("home");
  homeView = $state<HomeView>("recents");
  thumbnails = $state<Record<string, string>>({});
  ready = $state(false);
  saving = $state(false);
  /** The tab on screen has changes not yet written (refreshed by the autosave tick). */
  unsaved = $state(false);
  error = $state<string | null>(null);
  private closed: FileMeta[] = [];
  private nextSlot = 1;
  private timer: ReturnType<typeof setInterval> | null = null;

  get current(): Tab | null {
    return this.tabs.find((t) => t.slot === this.active) ?? null;
  }

  /** Loads the file list and restores the last session's tabs. */
  async init() {
    await this.refresh();
    let session: { tabs: string[]; active: string | null } = { tabs: [], active: null };
    try {
      session = JSON.parse(localStorage.getItem(SESSION) ?? "null") ?? session;
    } catch {
      // first run
    }
    if (this.files.length === 0) {
      // First launch: keep the sample the engine started with as a "Welcome" file.
      const engine = editor.engine;
      const meta = await files.create("Welcome to Pergel", engine?.docJson() ?? EMPTY_DOC);
      await this.refresh();
      await this.open(meta);
    } else {
      for (const id of session.tabs) {
        const meta = this.files.find((f) => f.id === id && !f.trashed);
        if (meta) await this.open(meta, false);
      }
      const active = this.tabs.find((t) => t.file.id === session.active);
      if (active) this.switchTo(active.slot);
      else if (!this.tabs.length) this.goHome();
    }
    editor.engine?.docClose(0);
    this.ready = true;
    this.timer ??= setInterval(() => void this.autosave(), AUTOSAVE_MS);
    window.addEventListener("beforeunload", () => this.persistSession());
  }

  async refresh() {
    try {
      this.files = await files.list();
      this.error = null;
    } catch (e) {
      this.error = String(e);
    }
  }

  async loadThumbnail(id: string) {
    const b64 = await files.thumbnail(id).catch(() => null);
    if (b64) this.thumbnails = { ...this.thumbnails, [id]: `data:image/png;base64,${b64}` };
  }

  private persistSession() {
    try {
      localStorage.setItem(
        SESSION,
        JSON.stringify({ tabs: this.tabs.map((t) => t.file.id), active: this.current?.file.id ?? null }),
      );
    } catch {
      // ignore
    }
  }

  /** Opens a file in a tab (or switches to its tab). */
  async open(meta: FileMeta, show = true) {
    const existing = this.tabs.find((t) => t.file.id === meta.id);
    if (existing) {
      if (show) this.switchTo(existing.slot);
      return;
    }
    const engine = editor.engine;
    if (!engine) return;
    let text: string;
    try {
      text = await files.read(meta.id);
    } catch (e) {
      editor.notify(`Could not open “${meta.name}”: ${e}`);
      return;
    }
    await this.saveCurrent();
    const slot = this.nextSlot++;
    if (!engine.docOpen(slot, text)) {
      editor.notify(`“${meta.name}” is not a Pergel file`);
      const back = this.current;
      if (back) engine.docSwitch(back.slot);
      return;
    }
    this.tabs.push({ slot, file: meta, saved: engine.docRevision() });
    this.active = slot;
    this.afterSwitch();
    // Files without a thumbnail yet (new, imported) get one now.
    if (!this.thumbnails[meta.id] && !(await files.thumbnail(meta.id).catch(() => null))) {
      await this.saveCurrent(true);
    }
    if (!show) {
      // Restoring a session: the last opened tab stays on screen until the active one is chosen.
    }
  }

  /** Creates an empty design file and opens it (Ctrl+T, "New design file"). */
  async newFile(name = "Untitled") {
    await this.createFrom(name, EMPTY_DOC);
  }

  /** Branching: a copy of the open file to work on separately, then merge back. */
  async createBranch(name: string) {
    const tab = this.current;
    const engine = editor.engine;
    if (!tab || !engine) return;
    if (engine.branch()) return editor.notify("This is already a branch; merge it first or branch from the main file");
    await this.saveCurrent(true);
    // Stable ids on the main's top-level layers, so the merge can match them later.
    const base = engine.docJsonLive("m");
    await this.saveCurrent(true);
    const doc = JSON.parse(base) as Record<string, unknown>;
    doc.branch = { of: tab.file.id, of_name: tab.file.name, base };
    await this.createFrom(`${tab.file.name} · ${name || "branch"}`, JSON.stringify(doc));
    editor.notify(`Branch “${name || "branch"}” created from ${tab.file.name}`);
  }

  /** Branching: brings the branch's changes into its main file (the main's own changes stay). */
  async mergeBranch() {
    const engine = editor.engine;
    const tab = this.current;
    const info = engine?.branch();
    if (!engine || !tab || !info) return;
    const main = this.files.find((f) => f.id === info.of);
    if (!main) return editor.notify("The main file is gone");
    const branchDoc = JSON.parse(engine.docJsonLive("b")) as Record<string, unknown>;
    delete branchDoc.branch;
    await this.saveCurrent(true);
    const mainTab = this.tabs.find((t) => t.file.id === info.of);
    if (mainTab) this.switchTo(mainTab.slot);
    else await this.open(main);
    const live = engine.docJsonLive("m");
    // The branch wins where both changed the same frame: merging is choosing the branch.
    const merged = engine.docMerge(info.base, live, JSON.stringify(branchDoc));
    if (!merged) return editor.notify("Could not merge the branch");
    await this.saveVersion(`Before merging “${tab.file.name}”`);
    engine.docReplace(merged.doc);
    editor.refreshAll();
    await this.saveCurrent(true);
    await this.saveVersion(`Merged “${tab.file.name}”`);
    // Mark the branch merged.
    this.switchTo(tab.slot);
    engine.setBranch({ ...info, merged: Date.now() });
    await this.saveCurrent(true);
    if (mainTab || main) this.switchTo(this.tabs.find((t) => t.file.id === info.of)?.slot ?? tab.slot);
    editor.notify(`Merged into ${main.name}`);
  }

  /** Version history: save the current document as a named version. */
  async saveVersion(label: string) {
    const tab = this.current;
    const engine = editor.engine;
    if (!tab || !engine) return null;
    await this.saveCurrent(true);
    return versions.save(tab.file.id, engine.docJson(), label);
  }

  /** Replaces the open document with an older version (the current one is kept as a version). */
  async restoreVersion(time: number) {
    const tab = this.current;
    const engine = editor.engine;
    if (!tab || !engine) return;
    const content = await versions.read(tab.file.id, time);
    await versions.save(tab.file.id, engine.docJson(), "Before restore");
    if (!engine.docOpen(tab.slot, content)) return editor.notify("That version could not be opened");
    tab.saved = -1;
    this.afterSwitch();
    await this.saveCurrent(true);
    editor.notify("Version restored");
  }

  /** Opens an older version as a separate file. */
  async openVersionCopy(time: number, label: string) {
    const tab = this.current;
    if (!tab) return;
    const content = await versions.read(tab.file.id, time);
    const when = new Date(time).toLocaleString();
    await this.createFrom(`${tab.file.name} (${label || when})`, content);
  }

  /** Saves `content` (`.pergel` JSON) as a new file and opens it. */
  async createFrom(name: string, content: string) {
    const meta = await files.create(name, content);
    await this.refresh();
    await this.open(meta);
    return meta;
  }

  switchTo(slot: number) {
    const engine = editor.engine;
    if (!engine || !this.tabs.some((t) => t.slot === slot)) return;
    if (this.active === slot) return;
    void this.saveCurrent();
    engine.docSwitch(slot);
    this.active = slot;
    this.afterSwitch();
  }

  goHome() {
    void this.saveCurrent();
    if (editor.editing >= 0) editor.finishEditing();
    this.active = "home";
    this.persistSession();
    void this.refresh();
  }

  private afterSwitch() {
    const tab = this.current;
    if (tab) editor.fileName = tab.file.name;
    editor.expanded = new Set();
    editor.refreshAll();
    this.persistSession();
    void editor.ensureDocFonts();
  }

  async closeTab(slot: number) {
    const i = this.tabs.findIndex((t) => t.slot === slot);
    if (i < 0) return;
    const tab = this.tabs[i];
    if (this.active === slot) {
      await this.saveCurrent();
      const next = this.tabs[i + 1] ?? this.tabs[i - 1];
      if (next) this.switchTo(next.slot);
      else this.goHome();
    } else {
      // Saving a background tab needs it on screen; tabs are saved when left, so it is clean.
    }
    editor.engine?.docClose(slot);
    this.tabs.splice(this.tabs.findIndex((t) => t.slot === slot), 1);
    this.closed.push(tab.file);
    this.persistSession();
  }

  /** Ctrl+Shift+T. */
  async reopenClosed() {
    const meta = this.closed.pop();
    if (!meta) return;
    await this.refresh();
    const fresh = this.files.find((f) => f.id === meta.id && !f.trashed);
    if (fresh) await this.open(fresh);
  }

  cycle(step: number) {
    if (!this.tabs.length) return;
    const i = this.tabs.findIndex((t) => t.slot === this.active);
    const next = this.tabs[(i + step + this.tabs.length) % this.tabs.length];
    this.switchTo(next.slot);
  }

  /** Saves the tab on screen if it changed (with a fresh thumbnail). */
  async saveCurrent(force = false) {
    const tab = this.current;
    const engine = editor.engine;
    if (!tab || !engine) return;
    const rev = engine.docRevision();
    if (!force && rev === tab.saved) return;
    this.saving = true;
    try {
      const json = engine.docJson();
      const shot = engine.capture(PAGE, 480);
      const thumb = shot ? shot.url.slice(shot.url.indexOf(",") + 1) : null;
      const meta = await files.save(tab.file.id, json, thumb);
      tab.saved = rev;
      tab.file = meta;
      if (thumb) this.thumbnails = { ...this.thumbnails, [meta.id]: `data:image/png;base64,${thumb}` };
      this.error = null;
    } catch (e) {
      this.error = `Saving failed: ${e}`;
    } finally {
      this.saving = false;
    }
  }

  private async autosave() {
    // Not in the middle of typing or dragging: wait for the edit to finish.
    this.unsaved = this.dirty;
    if (this.active === "home" || editor.editing >= 0 || editor.interacting) return;
    await this.saveCurrent();
    this.unsaved = this.dirty;
  }

  get dirty() {
    const tab = this.current;
    return !!tab && !!editor.engine && editor.engine.docRevision() !== tab.saved;
  }

  async update(id: string, change: FileChange) {
    const meta = await files.update(id, change);
    for (const t of this.tabs) if (t.file.id === id) t.file = meta;
    if (this.current?.file.id === id) editor.fileName = meta.name;
    await this.refresh();
    return meta;
  }

  async duplicate(id: string) {
    await files.duplicate(id);
    await this.refresh();
  }

  /** Moves to the Trash (closing its tab), or restores from it. */
  async trash(id: string, trashed = true) {
    const tab = this.tabs.find((t) => t.file.id === id);
    if (tab && trashed) await this.closeTab(tab.slot);
    await this.update(id, { trashed });
  }

  async deleteForever(id: string) {
    await files.remove(id);
    await this.refresh();
  }
}

export const workspace = new Workspace();
