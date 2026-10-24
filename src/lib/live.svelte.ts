import { invoke, isTauri } from "@tauri-apps/api/core";
import { editor } from "./editor.svelte";
import type { LivePatch as Patch, Stamps } from "./engine";

// Live collaboration: everyone in a session sees the others' cursors and the latest document.
// The desktop app relays over the local network (see src-tauri/src/live.rs); `local:<room>`
// links use a BroadcastChannel between windows of the same browser. Every top-level layer (and
// page order, variables, styles…) carries a stamp of who changed it last (core `merge.rs`):
// peers send each other only the parts they changed, each part is taken from the later stamp
// (edits in different frames all survive; two people changing the same frame at once end up
// with the same one), edits made while disconnected are merged when rejoining the same session,
// and undo reverts only this person's own changes. Everyone shares the part of the canvas they
// look at, so others can follow them (click their avatar), or be asked to (spotlight).

export interface Peer {
  id: string;
  name: string;
  color: string;
  x: number;
  y: number;
  page: number;
  seen: number;
  /** What they look at: their page and the canvas area in view. */
  view: { page: number; area: [number, number, number, number] } | null;
  /** They asked everyone to follow them. */
  spotlight: boolean;
}

type Msg =
  | { type: "hello" | "here" | "bye"; from: string; name: string; color: string; spotlight?: boolean }
  | { type: "view"; from: string; page: number; area: [number, number, number, number] }
  | { type: "spotlight"; from: string; on: boolean }
  | { type: "cursor"; from: string; name: string; color: string; x: number; y: number; page: number }
  | { type: "sync-request"; from: string }
  | { type: "doc"; from: string; doc: string; stamps?: Stamps }
  | { type: "patch"; from: string; patch: Patch };

type Incoming = { doc: string; stamps: Stamps } | { patch: Patch };

const COLORS = ["#F24822", "#0D99FF", "#14AE5C", "#9747FF", "#FFA629", "#E8448C", "#00B5CE", "#7A5AF8"];
const NAME_KEY = "pergel.liveName";
/** Undo steps kept in a session. */
const HISTORY = 100;

function randomId() {
  return Math.random().toString(36).slice(2, 10);
}

/** What goes to the others of a change: the new values only, with their stamps. */
function outgoing(change: Patch, stamps: Stamps): Patch {
  const parts: Patch["parts"] = {};
  const own: Stamps = {};
  for (const [key, [, after]] of Object.entries(change.parts)) {
    parts[key] = [null, after];
    if (stamps[key]) own[key] = stamps[key];
  }
  return { parts, stamps: own, images: change.images };
}

class Live {
  status = $state<"off" | "connecting" | "on">("off");
  error = $state("");
  /** The link for others (desktop host only). */
  shareUrl = $state("");
  hosting = $state(false);
  peers = $state<Record<string, Peer>>({});
  name = $state(this.savedName());
  /** Undo and redo steps of this person's own changes in the session. */
  undoSteps = $state(0);
  redoSteps = $state(0);
  /** The person whose view this window follows. */
  following = $state<string | null>(null);
  /** This person asked everyone to follow them. */
  spotlighting = $state(false);
  readonly id = randomId();
  readonly color = COLORS[Math.floor(Math.random() * COLORS.length)];

  private send: ((m: Msg) => void) | null = null;
  private close: (() => void) | null = null;
  private timers: number[] = [];
  private lastRev = -1;
  /** Documents and patches from the others, applied in order once the user isn't mid-gesture. */
  private inbox: Incoming[] = [];
  /** The session link the stamps below belong to. */
  private session = "";
  /** Who changed each part of the document last. */
  private stamps: Stamps = {};
  /** The document `stamps` describe (what this peer last sent or applied). */
  private sent = "";
  /** Whether this copy is part of the session's document yet: a peer that joins takes the
   *  session's document as it is first, so its own unrelated file doesn't merge in. */
  private synced = false;
  private undoStack: Patch[] = [];
  private redoStack: Patch[] = [];
  private lastCursor = 0;
  /** The view last sent, as text, and when. */
  private lastView = "";
  private lastViewAt = 0;
  /** The view this window last moved to while following; any other means the user moved. */
  private shown = "";
  /** A spotlight the user stepped out of, so the heartbeat doesn't pull them back in. */
  private dismissed = "";

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
    this.here();
  }

  private here() {
    this.post({ type: "here", from: this.id, name: this.name, color: this.color, spotlight: this.spotlighting });
  }

  /** Starts a session others can join (desktop: on the local network; browser: other tabs). */
  async host() {
    this.error = "";
    if (!isTauri()) return this.join("local:pergel");
    try {
      const info = await invoke<{ localUrl: string; shareUrl: string }>("live_start");
      this.hosting = true;
      this.shareUrl = info.shareUrl;
      // The host's document is the session's.
      this.join(info.localUrl, true);
    } catch (e) {
      this.error = String(e);
    }
  }

  join(url: string, authoritative = false) {
    this.disconnect(false);
    url = url.trim();
    // Rejoining the same session keeps the stamps, so work done in between merges in.
    if (url !== this.session) {
      this.session = url;
      this.stamps = {};
      this.sent = "";
      this.synced = false;
      this.undoStack = [];
      this.redoStack = [];
      this.counts();
    }
    if (authoritative) this.synced = true;
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
      ws = new WebSocket(url);
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
    const engine = editor.engine;
    this.lastRev = engine?.docRevision() ?? -1;
    if (this.synced && !this.sent && engine) this.sent = engine.docJsonLive(this.id);
    this.hook();
    this.post({ type: "hello", from: this.id, name: this.name, color: this.color });
    this.post({ type: "sync-request", from: this.id });
    // Browser-tab sessions have no host: when no other tab answers, this one starts the session.
    if (this.session.startsWith("local:")) {
      this.timers.push(
        window.setTimeout(() => {
          if (this.synced || !editor.engine) return;
          this.synced = true;
          this.sent = editor.engine.docJsonLive(this.id);
          this.hook();
        }, 1500),
      );
    }
    this.timers.push(window.setInterval(() => this.tick(), 300));
    this.timers.push(window.setInterval(() => this.watchView(), 100));
    // Presence: a heartbeat so others drop people who left without saying so.
    this.timers.push(window.setInterval(() => this.here(), 3000));
  }

  /** Shares this window's view when it changes, and notices the user moving while following. */
  private watchView() {
    const engine = editor.engine;
    const area = engine?.viewArea();
    if (!engine || !area) return;
    const page = Math.max(0, editor.pages.findIndex((p) => p.current));
    const text = JSON.stringify([page, area.map((v) => Math.round(v))]);
    if (this.following && this.shown && text !== this.shown) this.unfollow();
    const now = Date.now();
    if (text !== this.lastView || now - this.lastViewAt > 3000) {
      this.lastView = text;
      this.lastViewAt = now;
      this.post({ type: "view", from: this.id, page, area });
    }
  }

  /** Follows someone's view (Figma's "Follow"); again to stop. */
  toggleFollow(id: string) {
    if (this.following === id) this.unfollow();
    else this.follow(id);
  }

  follow(id: string) {
    if (!this.peers[id]) return;
    this.following = id;
    this.shown = "";
    this.showView(this.peers[id]);
  }

  unfollow() {
    const p = this.following ? this.peers[this.following] : null;
    if (p?.spotlight) this.dismissed = p.id;
    this.following = null;
    this.shown = "";
  }

  /** Moves this window to what `p` looks at. */
  private showView(p: Peer) {
    const engine = editor.engine;
    if (!engine || !p.view || this.busy()) return;
    const current = Math.max(0, editor.pages.findIndex((pg) => pg.current));
    if (p.view.page !== current && p.view.page < editor.pages.length) editor.switchPage(p.view.page);
    const [x, y, w, h] = p.view.area;
    engine.viewShow(x, y, w, h);
    editor.invalidate();
    const area = engine.viewArea();
    if (area) this.shown = JSON.stringify([Math.max(0, editor.pages.findIndex((pg) => pg.current)), area.map((v) => Math.round(v))]);
  }

  /** Asks everyone to follow this window (Figma's "Spotlight me"); again to stop. */
  toggleSpotlight() {
    this.spotlighting = !this.spotlighting;
    if (this.spotlighting) this.unfollow();
    this.post({ type: "spotlight", from: this.id, on: this.spotlighting });
  }

  /** Someone started or stopped a spotlight. */
  private spotlight(p: Peer, on: boolean) {
    if (on && !this.spotlighting && this.following !== p.id && this.dismissed !== p.id) {
      this.follow(p.id);
      editor.notify(`${p.name} is presenting; you're following them. Move the canvas to stop.`);
    } else if (!on) {
      if (this.following === p.id) this.unfollow();
      if (this.dismissed === p.id) this.dismissed = "";
    }
  }

  /** In a session, Undo and Redo revert this person's own changes (see `undo`). */
  private hook() {
    editor.liveHistory = this.status === "on" && this.synced ? this : null;
  }

  private counts() {
    this.undoSteps = this.undoStack.length;
    this.redoSteps = this.redoStack.length;
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
    if (this.synced && rev !== this.lastRev && !this.busy() && this.inbox.length === 0) {
      this.lastRev = rev;
      this.sendChanges();
    }
    const now = Date.now();
    const gone = Object.values(this.peers).filter((p) => now - p.seen > 10_000);
    if (gone.length) {
      const next = { ...this.peers };
      for (const p of gone) delete next[p.id];
      this.peers = next;
      if (this.following && !next[this.following]) this.unfollow();
    }
  }

  /** Stamps this peer's changes since the last document sent or applied (`doc` is now that
   *  document) and keeps them as an undo step unless `record` is off. Returns them, to send. */
  private stamp(doc: string, record = true): Patch | null {
    const engine = editor.engine;
    const before = this.sent;
    this.sent = doc;
    if (!engine || !before || before === doc) return null;
    const r = engine.docLocalChange(before, doc, this.stamps, this.id);
    if (!r || !Object.keys(r.change.parts).length) return null;
    this.stamps = r.stamps;
    if (record) {
      this.undoStack.push(r.change);
      if (this.undoStack.length > HISTORY) this.undoStack.shift();
      this.redoStack = [];
      this.counts();
    }
    return outgoing(r.change, r.stamps);
  }

  /** Sends what this peer changed since it last sent or applied a document. */
  private sendChanges(record = true) {
    const engine = editor.engine;
    if (!engine) return;
    const patch = this.stamp(engine.docJsonLive(this.id), record);
    if (patch) this.post({ type: "patch", from: this.id, patch });
  }

  /** Sends the whole document (to someone joining). */
  private sendDocument() {
    const engine = editor.engine;
    if (!engine) return;
    this.stamp(engine.docJsonLive(this.id));
    this.post({ type: "doc", from: this.id, doc: this.sent, stamps: this.stamps });
  }

  private upsert(id: string, change: Partial<Peer>) {
    const old = this.peers[id] ?? { id, name: "Guest", color: COLORS[0], x: NaN, y: NaN, page: 0, seen: 0, view: null, spotlight: false };
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
        this.here();
        editor.notify(`${m.name} joined`);
        break;
      case "here": {
        const was = this.peers[m.from]?.spotlight ?? false;
        this.upsert(m.from, { name: m.name, color: m.color, spotlight: !!m.spotlight });
        // Joining while someone presents: follow them too.
        if (!!m.spotlight !== was) this.spotlight(this.peers[m.from], !!m.spotlight);
        break;
      }
      case "bye": {
        const next = { ...this.peers };
        delete next[m.from];
        this.peers = next;
        if (this.following === m.from) this.unfollow();
        break;
      }
      case "view":
        this.upsert(m.from, { view: { page: m.page, area: m.area } });
        if (this.following === m.from) this.showView(this.peers[m.from]);
        break;
      case "spotlight":
        this.upsert(m.from, { spotlight: m.on });
        this.spotlight(this.peers[m.from], m.on);
        break;
      case "cursor":
        this.upsert(m.from, { name: m.name, color: m.color, x: m.x, y: m.y, page: m.page });
        break;
      case "sync-request":
        // Only a copy that is part of the session answers, so a newcomer gets the session's
        // document, not another newcomer's.
        if (this.synced) this.sendDocument();
        break;
      case "doc":
        this.upsert(m.from, {});
        this.inbox.push({ doc: m.doc, stamps: m.stamps ?? {} });
        this.apply();
        break;
      case "patch":
        this.upsert(m.from, {});
        // Kept until this copy has the session's document; stamps skip what it already holds.
        if (this.inbox.length < 1000) this.inbox.push({ patch: m.patch });
        this.apply();
        break;
    }
  }

  /** Shows a document in the editor: 1, or 0 while the user is mid-gesture, -1 if invalid. */
  private show(doc: string) {
    const engine = editor.engine;
    if (!engine) return 0;
    if (doc !== engine.docJson()) {
      const r = engine.docReplace(doc);
      if (r <= 0) return r;
      editor.refreshAll();
      editor.invalidate();
    }
    this.lastRev = engine.docRevision();
    this.sent = engine.docJsonLive(this.id);
    return 1;
  }

  /** Applies what the others sent once the local user is not mid-gesture. */
  private apply() {
    const engine = editor.engine;
    if (!engine || this.inbox.length === 0 || this.busy()) return;
    if (!this.synced) {
      // Joining: wait for the session's document, then apply what came before it on top.
      const first = this.inbox.findIndex((i) => "doc" in i);
      if (first < 0) return;
      this.inbox.unshift(...this.inbox.splice(first, 1));
    }
    // This peer's own unsent edits go out first, so the others merge them too.
    if (this.synced) this.sendChanges();
    let doc = this.sent || engine.docJsonLive(this.id);
    let stamps = this.stamps;
    let synced = this.synced;
    let ahead = false;
    for (const item of this.inbox) {
      if ("patch" in item) {
        const r = engine.docApplyPatch(doc, stamps, item.patch);
        if (r) ({ doc, stamps } = r);
      } else if (!synced) {
        // Joining: take the session's document as it is.
        ({ doc, stamps } = item);
        synced = true;
      } else {
        const r = engine.docMergeLive(doc, stamps, item.doc, item.stamps);
        if (r) {
          ({ doc, stamps } = r);
          ahead ||= r.ahead;
        }
      }
    }
    const shown = this.show(doc);
    // Busy: try again on the next tick. An invalid document is dropped.
    if (shown < 0) this.inbox = [];
    if (shown <= 0) return;
    this.inbox = [];
    this.stamps = stamps;
    if (!this.synced && synced) {
      this.synced = true;
      this.hook();
    }
    // Parts this peer had that the others' document lacked go back out.
    if (ahead) this.post({ type: "doc", from: this.id, doc: this.sent, stamps: this.stamps });
  }

  get canUndo() {
    return this.undoSteps > 0;
  }

  get canRedo() {
    return this.redoSteps > 0;
  }

  /** Undoes this person's last change, leaving the parts others changed since. */
  undo() {
    return this.step(false);
  }

  redo() {
    return this.step(true);
  }

  private step(redo: boolean) {
    const engine = editor.engine;
    if (!engine || !this.synced || this.busy()) return false;
    // Edits not sent yet are the latest change.
    this.sendChanges();
    const from = redo ? this.redoStack : this.undoStack;
    const to = redo ? this.undoStack : this.redoStack;
    while (from.length) {
      const change = from.pop()!;
      const r = engine.docRevert(this.sent, change, redo);
      // Others changed all of it since: on to the step before.
      if (!r || r.changed === 0) continue;
      const before = this.sent;
      if (this.show(r.doc) <= 0) {
        from.push(change);
        this.counts();
        return false;
      }
      to.push(change);
      this.counts();
      // The others see the undo like any other change; it isn't a new undo step itself.
      const after = this.sent;
      this.sent = before;
      const patch = this.stamp(after, false);
      if (patch) this.post({ type: "patch", from: this.id, patch });
      return true;
    }
    this.counts();
    return false;
  }

  /** The local pointer, in canvas coordinates (sent at most 25 times a second). */
  cursor(x: number, y: number) {
    if (this.status !== "on") return;
    const now = performance.now();
    if (now - this.lastCursor < 40) return;
    this.lastCursor = now;
    this.post({ type: "cursor", from: this.id, name: this.name, color: this.color, x, y, page: Math.max(0, editor.pages.findIndex((p) => p.current)) });
  }

  /** Leaves the session (and stops sharing when this window hosts it). Rejoining the same link
   *  later merges the work done in between. */
  disconnect(stopHosting = true, error = "") {
    if (this.status !== "off") this.post({ type: "bye", from: this.id, name: this.name, color: this.color });
    this.timers.forEach(clearInterval);
    this.timers = [];
    this.close?.();
    this.close = null;
    this.send = null;
    this.status = "off";
    this.peers = {};
    this.inbox = [];
    this.error = error;
    this.following = null;
    this.spotlighting = false;
    this.dismissed = "";
    this.lastView = "";
    this.hook();
    if (stopHosting && this.hosting) {
      this.hosting = false;
      this.shareUrl = "";
      if (isTauri()) void invoke("live_stop");
    }
  }
}

export const live = new Live();
