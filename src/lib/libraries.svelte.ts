import { editor } from "./editor.svelte";
import { workspace } from "./workspace.svelte";

// Team libraries: a file publishes its components (and component sets); any other file can use
// them. A used component is copied into the file once (managed, off canvas) and its instances
// follow it; publishing again offers the update everywhere. Libraries live with Pergel's local
// data, shared by every file on this computer.

export interface LibraryComponent {
  key: string;
  name: string;
  set: boolean;
  w: number;
  h: number;
  tree: unknown;
  thumbnail: string | null;
}

export interface Library {
  id: string;
  name: string;
  version: number;
  components: LibraryComponent[];
}

const KEY = "pergel.libraries";

function load(): Record<string, Library> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, Library>;
  } catch {
    return {};
  }
}

class Libraries {
  all = $state<Record<string, Library>>(load());
  /** Library components used in the open file: `{lib, key, version, id, name}`. */
  refs = $state<{ lib: string; key: string; version: number; id: number; name: string }[]>([]);

  private save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.all));
    } catch {
      editor.notify("Not enough space to store the library");
    }
  }

  refresh() {
    const engine = editor.engine;
    if (!engine) return;
    this.refs = engine.libraryRefs();
  }

  /** The library ids with a newer version than the one this file uses. */
  get updates() {
    return this.refs.filter((r) => (this.all[r.lib]?.version ?? 0) > r.version);
  }

  /** Publishes the open file's components as a library (again: a new version). */
  publish() {
    const engine = editor.engine;
    const tab = workspace.current;
    if (!engine) return 0;
    const id = tab?.file.id ?? "local";
    const name = tab?.file.name ?? editor.fileName;
    const pkg = engine.libraryPackage("lib");
    if (!pkg.components.length) {
      editor.notify("No components to publish (Ctrl+Alt+K makes one)");
      return 0;
    }
    const components: LibraryComponent[] = pkg.components.map((c) => ({
      key: c.key,
      name: c.name,
      set: c.set,
      w: c.w,
      h: c.h,
      tree: c.tree,
      thumbnail: c.nodeId === null ? null : engine.thumbnail(c.nodeId, 128),
    }));
    this.all = { ...this.all, [id]: { id, name, version: Date.now(), components } };
    this.save();
    editor.notify(`Published ${components.length} component${components.length === 1 ? "" : "s"} from ${name}`);
    return components.length;
  }

  unpublish(id: string) {
    const next = { ...this.all };
    delete next[id];
    this.all = next;
    this.save();
  }

  /** Places an instance of a library component; returns its id or -1. */
  use(libId: string, key: string, at?: { x: number; y: number }) {
    const engine = editor.engine;
    const lib = this.all[libId];
    const c = lib?.components.find((x) => x.key === key || x.name.toLowerCase() === key.toLowerCase());
    if (!engine || !lib || !c) return -1;
    const id = engine.libraryUse({ lib: lib.id, libName: lib.name, key: c.key, version: lib.version, tree: c.tree, x: at?.x, y: at?.y });
    editor.refreshAll();
    editor.refreshComponents();
    this.refresh();
    return id;
  }

  /** Accepts every available update. */
  updateAll() {
    const engine = editor.engine;
    if (!engine) return 0;
    let moved = 0;
    for (const r of this.updates) {
      const lib = this.all[r.lib];
      const c = lib?.components.find((x) => x.key === r.key);
      if (!lib || !c) continue;
      moved += Math.max(0, engine.libraryUpdate({ lib: lib.id, libName: lib.name, key: c.key, version: lib.version, tree: c.tree }));
    }
    editor.refreshAll();
    editor.refreshComponents();
    this.refresh();
    editor.notify(`Updated ${moved} instance${moved === 1 ? "" : "s"} to the latest library`);
    return moved;
  }
}

export const libraries = new Libraries();
