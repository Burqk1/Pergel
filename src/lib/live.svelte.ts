import { invoke, isTauri } from "@tauri-apps/api/core";
import { editor } from "./editor.svelte";

// Live collaboration: everyone in a session sees the others' cursors and the latest document.
// The desktop app relays over the local network (see src-tauri/src/live.rs); `local:<room>`
// links use a BroadcastChannel between windows of the same browser. Documents merge three-way
// per top-level layer (core `merge_documents`): edits in different frames all survive; when two
// people change the same frame at once, the later change wins.

export interface Peer {
  id: string;
  name: string;
  color: string;
  x: number;
  y: number;
  page: number;
  seen: number;
}

type Msg =
  | { type: "hello" | "here" | "bye"; from: string; name: string; color: string }
  | { type: "cursor"; from: string; name: string; color: string; x: number; y: number; page: number }
  | { type: "sync-request"; from: string }
  | { type: "doc"; from: string; doc: string };

const COLORS = ["#F24822", "#0D99FF", "#14AE5C", "#9747FF", "#FFA629", "#E8448C", "#00B5CE", "#7A5AF8"];
const NAME_KEY = "pergel.liveName";

function randomId() {
  return Math.random().toString(36).slice(2, 10);
}

class Live {
  status = $state<"off" | "connecting" | "on">("off");
  error = $state("");
  /** The link for others (desktop host only). */
  shareUrl = $state("");
  hosting = $state(false);
  peers = $state<Record<string, Peer>>({});
  name = $state(this.savedName());
  readonly id = randomId();
  readonly color = COLORS[Math.floor(Math.random() * COLORS.length)];

  private send: ((m: Msg) => void) | null = null;
  private close: (() => void) | null = null;
  private timers: number[] = [];
  private lastRev = -1;
  private pending: string | null = null;
  /** The last document everyone agreed on (what this peer sent or applied). */
  private base = "";
  private lastCursor = 0;

  private savedName() {
    try {
      return localStorage.getItem(NAME_KEY) || `Guest ${Math.floor(Math.random() * 900 + 100)}`;
    } catch {
      return "Guest";
    }
  }

  setName(name: string) {
    this.name = name.trim() || this.name;
    try {
      localStorage.setItem(NAME_KEY, this.name);
    } catch {
      // not persisted
    }
    this.post({ type: "here", from: this.id, name: this.name, color: this.color });
  }

  /** Starts a session others can join (desktop: on the local network; browser: other tabs). */
  async host() {
    this.error = "";
    if (!isTauri()) return this.join("local:pergel");
    try {
      const info = await invoke<{ localUrl: string; shareUrl: string }>("live_start");
      this.hosting = true;
      this.shareUrl = info.shareUrl;
      this.join(info.localUrl);
    } catch (e) {
      this.error = String(e);
    }
  }

  join(url: string) {
    this.disconnect(false);
    this.error = "";
    this.status = "connecting";
    if (url.startsWith("local:")) {
      const channel = new BroadcastChannel(`pergel-live-${url.slice(6)}`);
      channel.onmessage = (e) => this.receive(String(e.data));
      this.send = (m) => channel.postMessage(JSON.stringify(m));
      this.close = () => channel.close();
      this.shareUrl ||= url;
      this.opened();
      return;
    }
    let ws: WebSocket;
    try {
      ws = new WebSocket(url.trim());
    } catch {
      this.status = "off";
      this.error = "That is not a session link (ws://…)";
      return;
    }
    ws.onopen = () => this.opened();
    ws.onmessage = (e) => this.receive(String(e.data));
    ws.onerror = () => (this.error = "Could not reach the session");
    ws.onclose = () => {
      if (this.status !== "off") this.disconnect(false, "The session ended");
    };
    this.send = (m) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(m));
    };
    this.close = () => ws.close();
  }

  private opened() {
    this.status = "on";
    this.lastRev = editor.engine?.docRevision() ?? -1;
    this.post({ type: "hello", from: this.id, name: this.name, color: this.color });
    this.post({ type: "sync-request", from: this.id });
    this.timers.push(window.setInterval(() => this.tick(), 300));
    // Presence: a heartbeat so others drop people who left without saying so.
    this.timers.push(window.setInterval(() => this.post({ type: "here", from: this.id, name: this.name, color: this.color }), 3000));
  }

  private post(m: Msg) {
    this.send?.(m);
  }

  private busy() {
    return editor.interacting || editor.editing >= 0 || editor.vectorMode > 0;
  }

  private tick() {
    const engine = editor.engine;
    if (!engine) return;
    this.apply();
    const rev = engine.docRevision();
    if (rev !== this.lastRev && !this.busy() && this.pending === null) {
      this.lastRev = rev;
      this.base = engine.docJsonLive(this.id);
      this.post({ type: "doc", from: this.id, doc: this.base });
    }
    const now = Date.now();
    const gone = Object.values(this.peers).filter((p) => now - p.seen > 10_000);
    if (gone.length) {
      const next = { ...this.peers };
      for (const p of gone) delete next[p.id];
      this.peers = next;
    }
  }

  private upsert(id: string, change: Partial<Peer>) {
    const old = this.peers[id] ?? { id, name: "Guest", color: COLORS[0], x: NaN, y: NaN, page: 0, seen: 0 };
    this.peers = { ...this.peers, [id]: { ...old, ...change, seen: Date.now() } };
  }

  private receive(raw: string) {
    let m: Msg;
    try {
      m = JSON.parse(raw) as Msg;
    } catch {
      return;
    }
    if (!m || m.from === this.id) return;
    switch (m.type) {
      case "hello":
        this.upsert(m.from, { name: m.name, color: m.color });
        this.post({ type: "here", from: this.id, name: this.name, color: this.color });
        editor.notify(`${m.name} joined`);
        break;
      case "here":
        this.upsert(m.from, { name: m.name, color: m.color });
        break;
      case "bye": {
        const next = { ...this.peers };
        delete next[m.from];
        this.peers = next;
        break;
      }
      case "cursor":
        this.upsert(m.from, { name: m.name, color: m.color, x: m.x, y: m.y, page: m.page });
        break;
      case "sync-request":
        if (editor.engine) {
          this.base = editor.engine.docJsonLive(this.id);
          this.post({ type: "doc", from: this.id, doc: this.base });
        }
        break;
      case "doc":
        this.upsert(m.from, {});
        this.pending = m.doc;
        this.apply();
        break;
    }
  }

  /** Shows the newest document from the others once the local user is not mid-gesture. */
  private apply() {
    const engine = editor.engine;
    if (!engine || this.pending === null || this.busy()) return;
    const remote = this.pending;
    // Joining: take the session's document as it is. Afterwards: merge with local edits.
    const merged = this.base ? engine.docMerge(this.base, engine.docJsonLive(this.id), remote) : null;
    const doc = merged?.doc ?? remote;
    if (doc === engine.docJson()) {
      this.pending = null;
      this.base = remote;
      return;
    }
    const r = engine.docReplace(doc);
    if (r === 0) return;
    this.pending = null;
    this.lastRev = engine.docRevision();
    this.base = merged?.ahead ? doc : remote;
    // Local edits the others don't have yet go out with the merged document.
    if (merged?.ahead) this.post({ type: "doc", from: this.id, doc });
    if (r === 1) {
      editor.refreshAll();
      editor.invalidate();
    }
  }

  /** The local pointer, in canvas coordinates (sent at most 25 times a second). */
  cursor(x: number, y: number) {
    if (this.status !== "on") return;
    const now = performance.now();
    if (now - this.lastCursor < 40) return;
    this.lastCursor = now;
    this.post({ type: "cursor", from: this.id, name: this.name, color: this.color, x, y, page: Math.max(0, editor.pages.findIndex((p) => p.current)) });
  }

  /** Leaves the session (and stops sharing when this window hosts it). */
  disconnect(stopHosting = true, error = "") {
    if (this.status !== "off") this.post({ type: "bye", from: this.id, name: this.name, color: this.color });
    this.timers.forEach(clearInterval);
    this.timers = [];
    this.close?.();
    this.close = null;
    this.send = null;
    this.status = "off";
    this.peers = {};
    this.pending = null;
    this.base = "";
    this.error = error;
    if (stopHosting && this.hosting) {
      this.hosting = false;
      this.shareUrl = "";
      if (isTauri()) void invoke("live_stop");
    }
  }
}

export const live = new Live();
