import { invoke, isTauri } from "@tauri-apps/api/core";

/** A stored document, as listed on the Home screen. */
export interface FileMeta {
  id: string;
  name: string;
  /** Milliseconds since the epoch. */
  created: number;
  modified: number;
  opened: number;
  starred: boolean;
  trashed: boolean;
}

export interface FileChange {
  name?: string;
  starred?: boolean;
  trashed?: boolean;
}

/** Where documents live: the app data folder on desktop, localStorage in a plain browser. */
interface Backend {
  list(): Promise<FileMeta[]>;
  create(name: string, content: string): Promise<FileMeta>;
  read(id: string): Promise<string>;
  save(id: string, content: string, thumbnail: string | null): Promise<FileMeta>;
  thumbnail(id: string): Promise<string | null>;
  update(id: string, change: FileChange): Promise<FileMeta>;
  duplicate(id: string): Promise<FileMeta>;
  remove(id: string): Promise<void>;
}

const desktop: Backend = {
  list: () => invoke("files_list"),
  create: (name, content) => invoke("file_create", { name, content }),
  read: (id) => invoke("file_read", { id }),
  save: (id, content, thumbnail) => invoke("file_save", { id, content, thumbnail }),
  thumbnail: (id) => invoke("file_thumbnail", { id }),
  update: (id, change) => invoke("file_update", { id, change }),
  duplicate: (id) => invoke("file_duplicate", { id }),
  remove: (id) => invoke("file_delete", { id }),
};

const KEY = "pergel.files";

function readIndex(): FileMeta[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]") as FileMeta[];
  } catch {
    return [];
  }
}

function writeIndex(files: FileMeta[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(files));
  } catch {
    // Storage full or blocked: the browser fallback is best effort.
  }
}

function edit(id: string, f: (m: FileMeta) => void): FileMeta {
  const files = readIndex();
  const meta = files.find((m) => m.id === id);
  if (!meta) throw new Error(`No file ${id}`);
  f(meta);
  writeIndex(files);
  return { ...meta };
}

const browser: Backend = {
  async list() {
    return readIndex();
  },
  async create(name, content) {
    const t = Date.now();
    const meta: FileMeta = {
      id: t.toString(16) + Math.random().toString(16).slice(2, 6),
      name,
      created: t,
      modified: t,
      opened: t,
      starred: false,
      trashed: false,
    };
    try {
      localStorage.setItem(`pergel.file.${meta.id}`, content);
    } catch {
      // ignore
    }
    writeIndex([...readIndex(), meta]);
    return meta;
  },
  async read(id) {
    edit(id, (m) => (m.opened = Date.now()));
    const text = localStorage.getItem(`pergel.file.${id}`);
    if (text === null) throw new Error("The file's contents are missing");
    return text;
  },
  async save(id, content, thumbnail) {
    try {
      localStorage.setItem(`pergel.file.${id}`, content);
      if (thumbnail) localStorage.setItem(`pergel.thumb.${id}`, thumbnail);
    } catch {
      // ignore
    }
    return edit(id, (m) => (m.modified = Date.now()));
  },
  async thumbnail(id) {
    return localStorage.getItem(`pergel.thumb.${id}`);
  },
  async update(id, change) {
    return edit(id, (m) => {
      if (change.name?.trim()) m.name = change.name.trim();
      if (change.starred !== undefined) m.starred = change.starred;
      if (change.trashed !== undefined) m.trashed = change.trashed;
    });
  },
  async duplicate(id) {
    const src = readIndex().find((m) => m.id === id);
    if (!src) throw new Error(`No file ${id}`);
    const copy = await browser.create(`${src.name} (Copy)`, await browser.read(id));
    const thumb = localStorage.getItem(`pergel.thumb.${id}`);
    if (thumb) localStorage.setItem(`pergel.thumb.${copy.id}`, thumb);
    return copy;
  },
  async remove(id) {
    writeIndex(readIndex().filter((m) => m.id !== id));
    localStorage.removeItem(`pergel.file.${id}`);
    localStorage.removeItem(`pergel.thumb.${id}`);
  },
};

export const files: Backend = isTauri() ? desktop : browser;

export interface Version {
  time: number;
  label: string;
  size: number;
}

/** Version history (desktop app; the browser keeps none). */
export const versions = {
  list: (id: string): Promise<Version[]> => (isTauri() ? invoke<Version[]>("file_versions", { id }) : Promise.resolve([])),
  save: (id: string, content: string, label: string): Promise<Version | null> =>
    isTauri() ? invoke<Version>("file_version_save", { id, content, label }) : Promise.resolve(null),
  read: (id: string, time: number): Promise<string> => invoke<string>("file_version_read", { id, time }),
};

/** Picks a `.pergel` file on disk and copies it into the store (desktop only). */
export async function importFromDisk(): Promise<FileMeta | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ multiple: false, filters: [{ name: "Pergel file", extensions: ["pergel"] }] });
  if (typeof path !== "string") return null;
  return invoke<FileMeta>("file_import", { path });
}

/** Saves a stored file to a place on disk the user picks (desktop only). */
export async function exportToDisk(meta: FileMeta): Promise<boolean> {
  if (!isTauri()) return false;
  const { save } = await import("@tauri-apps/plugin-dialog");
  const path = await save({ defaultPath: `${meta.name}.pergel`, filters: [{ name: "Pergel file", extensions: ["pergel"] }] });
  if (!path) return false;
  await invoke("file_export", { id: meta.id, path });
  return true;
}

/** Saves bytes (base64) or text to a path the user picks (desktop), or downloads it (browser). */
export async function saveAs(name: string, ext: string, data: { base64?: string; text?: string }): Promise<boolean> {
  if (isTauri()) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({ defaultPath: `${name}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!path) return false;
    await invoke("write_file", { path, base64: data.base64 ?? null, text: data.text ?? null });
    return true;
  }
  const a = document.createElement("a");
  a.download = `${name}.${ext}`;
  a.href = data.base64
    ? `data:application/octet-stream;base64,${data.base64}`
    : URL.createObjectURL(new Blob([data.text ?? ""]));
  a.click();
  return true;
}

/** "2 hours ago", like Figma's file cards. */
export function ago(ms: number): string {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  const units: [number, string][] = [
    [60 * 60 * 24 * 365, "year"],
    [60 * 60 * 24 * 30, "month"],
    [60 * 60 * 24 * 7, "week"],
    [60 * 60 * 24, "day"],
    [60 * 60, "hour"],
    [60, "minute"],
  ];
  for (const [size, unit] of units) {
    const n = Math.floor(s / size);
    if (n >= 1) return `${n} ${unit}${n === 1 ? "" : "s"} ago`;
  }
  return "just now";
}
