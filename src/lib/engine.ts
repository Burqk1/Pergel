export type VarKind = "color" | "number" | "string" | "boolean";
/** Live collaboration: who last changed each part of the document (Lamport clock, peer id). */
export type Stamps = Record<string, [number, string]>;
/** Live collaboration: changed document parts with their old and new values (null: absent). */
export interface LivePatch {
  parts: Record<string, [unknown, unknown]>;
  stamps?: Stamps;
  images?: Record<string, string>;
}
export type VarValue =
  | { kind: "color"; value: number }
  | { kind: "number"; value: number }
  | { kind: "string"; value: string }
  | { kind: "boolean"; value: boolean }
  | { kind: "alias"; value: string };
export interface VarMode {
  id: string;
  name: string;
}
export interface VarCollection {
  id: string;
  name: string;
  modes: VarMode[];
  active: string;
}
export interface Variable {
  id: string;
  name: string;
  collection: string;
  type: VarKind;
  values: Record<string, VarValue>;
  description?: string;
}
export interface Variables {
  collections: VarCollection[];
  variables: Variable[];
}
export type BindProp =
  | "fill"
  | "stroke"
  | "strokeWidth"
  | "radius"
  | "opacity"
  | "gap"
  | "padding"
  | "width"
  | "height"
  | "fontSize"
  | "text"
  | "visible";

export type Trigger = { type: "on-click" } | { type: "on-hover" } | { type: "after-delay"; ms: number };
export type OverlayPosition = "center" | "top" | "bottom" | "left" | "right";
export type Action =
  | { type: "navigate"; to: number }
  | { type: "back" }
  | { type: "open-url"; url: string }
  | { type: "change-to"; to: number }
  | { type: "open-overlay"; to: number; position: OverlayPosition; closeOutside: boolean; dim: boolean }
  | { type: "close-overlay" }
  | { type: "set-variable"; variable: string; value: VarValue }
  | { type: "conditional"; variable: string; equals: VarValue; then: Action[]; otherwise: Action[] }
  | { type: "scroll-to"; to: number };
export type Transition = "instant" | "dissolve" | "smart-animate" | "slide-left" | "slide-right" | "slide-up" | "slide-down" | "push";
export interface Reaction {
  trigger: Trigger;
  action: Action;
  transition: Transition;
  duration: number;
}
export interface Hotspot {
  id: number;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  reactions: Reaction[];
}

export type StyleKind = "text" | "fill" | "stroke" | "effect";
export interface TypeStyleInfo {
  family: string;
  weight: number;
  italic: boolean;
  size: number;
  lineHeight: LineHeight;
  letterSpacing: number;
}
export type StyleValue =
  | ({ type: "text" } & TypeStyleInfo)
  | { type: "color"; color: number; paint?: Paint }
  | { type: "effect"; effects: unknown[] };
export type Style = { id: string; name: string; description?: string } & StyleValue;
export interface Styles {
  list: Style[];
}
export interface StyleLinks {
  text?: string;
  fill?: string;
  stroke?: string;
  effect?: string;
}

/** What a Figma import brought in. */
export interface FigmaReport {
  pages: number;
  layers: number;
  images: number;
  missing_images: number;
  components: number;
  fonts: string[];
}

/** One extra fill: a colour (opacity 0–100) or a gradient/image paint. */
export interface ExtraFill {
  color: string;
  opacity: number;
  paint: Paint | null;
  visible: boolean;
}
export interface BranchInfo {
  /** The main file's id and name. */
  of: string;
  of_name: string;
  /** The main document when the branch was made. */
  base: string;
  /** When it was merged into the main file. */
  merged?: number;
}
export interface CommentThread {
  id: string;
  /** Canvas position of the pin. */
  x: number;
  y: number;
  /** The layer it is pinned to. */
  node: number | null;
  nodeName: string | null;
  resolved: boolean;
  messages: { author: string; text: string; time: number }[];
}
export type PropKind = "boolean" | "text" | "instance-swap";
export type PropValue = boolean | string | number;
export interface ComponentProp {
  id: string;
  name: string;
  kind: PropKind;
  default: PropValue;
}
export interface ComponentPropInfo extends ComponentProp {
  /** The instance's value (instances only). */
  value?: PropValue;
}
export interface PropRefs {
  visible?: string;
  text?: string;
  swap?: string;
}
export type Scroll = "none" | "vertical" | "horizontal" | "both";
export type EndPoint = "none" | "line-arrow" | "triangle-arrow" | "reversed-triangle" | "circle" | "diamond";
export type NodeKind = "Frame" | "Group" | "Rectangle" | "Ellipse" | "Text" | "Vector" | "Image" | "Mask" | "Section" | "Slice";

export type GradientKind = "linear" | "radial" | "angular" | "diamond";
export interface GradientStop {
  /** 0–1. */
  at: number;
  /** Packed ARGB, as the engine stores colours. */
  color: number;
}
/** Handles are in the layer's unit square; `width` is the second axis relative to the first. */
export interface Gradient {
  type: "gradient";
  kind: GradientKind;
  stops: GradientStop[];
  from: [number, number];
  to: [number, number];
  width: number;
}
export type ImageMode = "fill" | "fit" | "crop" | "tile";
export interface ImagePaint {
  type: "image";
  hash: string;
  mode: ImageMode;
  width: number;
  height: number;
  scale: number;
  opacity: number;
  /** Figma's image adjustments, each −1…1. */
  adjust?: Partial<Record<ImageAdjustKey, number>>;
}
export type ImageAdjustKey = "exposure" | "contrast" | "saturation" | "temperature" | "tint" | "highlights" | "shadows";
/** A fill that is not a plain colour. */
export type Paint = Gradient | ImagePaint;

export type LineHeight = { unit: "auto" } | { unit: "px"; value: number } | { unit: "percent"; value: number };

/** Typography of a text layer, as stored by the engine (Figma's Text section). */
export interface TextStyle {
  content: string;
  family: string;
  weight: number;
  italic: boolean;
  size: number;
  lineHeight: LineHeight;
  /** Percent of the font size. */
  letterSpacing: number;
  align: "left" | "center" | "right" | "justify";
  vertical: "top" | "middle" | "bottom";
  resize: "auto-width" | "auto-height" | "fixed";
  decoration: "none" | "underline" | "strikethrough";
  case: "original" | "upper" | "lower" | "title";
  autoName: boolean;
  /** Bulleted or numbered lines. */
  list?: "none" | "bullet" | "numbered";
  /** OpenType features: { tnum: 1, liga: 0 }. */
  features?: Record<string, number>;
  /** Styled ranges (mixed styles). */
  runs?: { start: number; end: number; weight?: number; italic?: boolean; size?: number; family?: string; color?: number; decoration?: string; link?: string }[];
}

export interface TextInfo extends TextStyle {
  weightName: string;
  linePx: number;
  textHeight: number;
}

export type Blend =
  | "pass-through"
  | "normal"
  | "darken"
  | "multiply"
  | "color-burn"
  | "lighten"
  | "screen"
  | "color-dodge"
  | "overlay"
  | "soft-light"
  | "hard-light"
  | "difference"
  | "exclusion"
  | "hue"
  | "saturation"
  | "color"
  | "luminosity";

export type StrokeAlign = "inside" | "center" | "outside";

export type Flow = "horizontal" | "vertical" | "wrap" | "grid";
export type Justify = "start" | "center" | "end" | "space-between";
export type Align = "start" | "center" | "end" | "baseline";
export type Sizing = "fixed" | "hug" | "fill";

export type EffectType = "drop-shadow" | "inner-shadow" | "layer-blur" | "background-blur";

/** A layer effect; blurs use `blur` as their radius and ignore the colour and offsets. */
export interface EffectInfo {
  type: EffectType;
  color: string;
  /** Percent. */
  opacity: number;
  x: number;
  y: number;
  blur: number;
  spread: number;
  visible: boolean;
}

export type ExportFormat = "png" | "jpg" | "svg" | "pdf";

/** One Export row ("2x PNG"). `width`/`height` fix the size instead of `scale`. */
export interface ExportSetting {
  scale: number;
  width: number | null;
  height: number | null;
  format: ExportFormat;
  suffix: string;
}

/** A frame's auto layout (Figma's Shift+A). */
export interface AutoLayout {
  flow: Flow;
  gap: number;
  counterGap: number;
  /** Top, right, bottom, left. */
  padding: [number, number, number, number];
  justify: Justify;
  align: Align;
  columns: number;
  rows: number;
}

/** A partial property change applied to one layer or the whole selection (one undo step). */
export interface Patch {
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  rotation?: number;
  radius?: number;
  /** Four independent corners, or [] to go back to one radius. */
  corners?: number[];
  fill?: string;
  fillOpacity?: number;
  fillOn?: boolean;
  strokeOn?: boolean;
  strokeColor?: string;
  strokeOpacity?: number;
  strokeWidth?: number;
  strokeAlign?: StrokeAlign;
  /** Layer opacity in percent. */
  opacity?: number;
  blend?: Blend;
  lockRatio?: boolean;
  flow?: Flow;
  gap?: number;
  counterGap?: number;
  padding?: number[];
  justify?: Justify;
  align?: Align;
  columns?: number;
  rows?: number;
  sizingH?: Sizing;
  sizingV?: Sizing;
  absolute?: boolean;
  effects?: EffectInfo[];
  exports?: ExportSetting[];
  /** A gradient or image fill; `null` goes back to the solid colour. */
  paint?: Paint | null;
  constraintH?: Constraint;
  constraintV?: Constraint;
  grids?: LayoutGrid[];
  /** [dash, gap]; [] makes the stroke solid. */
  dash?: number[];
  /** Annotation; "" removes it. */
  note?: string;
  /** "Ready for dev". */
  ready?: boolean;
  startPoint?: EndPoint;
  endPoint?: EndPoint;
  scroll?: Scroll;
  /** [top, right, bottom, left]; [] turns individual strokes off. */
  strokeSides?: number[];
  /** Fills above the own fill, bottom to top (replaces the list). */
  extraFills?: ExtraFill[];
  /** Main or set: the property definitions. */
  componentProps?: ComponentProp[];
  /** Layer in a component: the properties it shows. */
  propRefs?: PropRefs;
  /** Instance: values by property id (null resets to the default). */
  propValues?: Record<string, boolean | string | number | null>;
}

export interface LayoutGrid {
  kind: "columns" | "rows" | "grid";
  count: number;
  gutter: number;
  margin: number;
  size: number;
  align: "stretch" | "start" | "center" | "end";
  /** Packed ARGB. */
  color: number;
  visible: boolean;
}

export interface FontFamily {
  name: string;
  weights: [number, number];
  upright: boolean;
  italic: boolean;
}

export interface NodeInfo {
  id: number;
  name: string;
  kind: NodeKind;
  parent: number;
  x: number;
  y: number;
  w: number;
  h: number;
  rotation: number;
  radius: number;
  corners: [number, number, number, number] | null;
  fill: string;
  fillOpacity: number;
  fillOn: boolean;
  stroke: { color: string; opacity: number; width: number; align: StrokeAlign } | null;
  /** Layer opacity in percent. */
  opacity: number;
  blend: Blend;
  lockRatio: boolean;
  layout: AutoLayout | null;
  /** Horizontal and vertical resizing as the layout applies it. */
  sizing: [Sizing, Sizing];
  absolute: boolean;
  /** Flow of the auto-layout parent placing this layer, if any. */
  parentFlow: Flow | null;
  effects: EffectInfo[];
  exports: ExportSetting[];
  component: "main" | "instance" | null;
  /** For an instance: its main component. */
  main: number | null;
  mainName: string | null;
  /** Inside an instance: can't be moved or deleted, only restyled. */
  inInstance: boolean;
  overrides: number;
  visible: boolean;
  locked: boolean;
  clip: boolean;
  children: number;
  text: TextInfo | null;
  /** A gradient or image fill replacing the solid colour. */
  paint: Paint | null;
  /** "Use as mask". */
  mask: boolean;
  /** Properties bound to variables (variable ids). */
  bindings: Partial<Record<BindProp, string>> | null;
  /** Horizontal, vertical. */
  constraints: [Constraint, Constraint];
  /** Linked styles (ids). */
  styles: StyleLinks | null;
  /** A component set (variants container). */
  variantSet: boolean;
  /** Instances of a variant: its properties. */
  variants: { name: string; values: string[]; current: string | null }[] | null;
  /** Prototype interactions. */
  reactions: Reaction[];
  grids: LayoutGrid[];
  dash: [number, number] | null;
  /** Designer's annotation. */
  note: string | null;
  ready: boolean;
  /** A section (Shift+S). */
  section: boolean;
  /** Start and end points of an open vector path. */
  endPoints: [EndPoint, EndPoint] | null;
  /** Prototype overflow scrolling (frames). */
  scroll: Scroll;
  /** Individual stroke weights: top, right, bottom, left. */
  sides: [number, number, number, number] | null;
  /** Fills above the own fill, bottom to top. */
  extraFills: ExtraFill[];
  /** Component properties: "define" on a main or set, "values" on an instance, "link" on a layer in a component. */
  componentProps: ComponentPropInfo[] | null;
  propsMode: "define" | "values" | "link" | null;
  propsOwner: number | null;
  propRefs: PropRefs | null;
  /** Code Connect mapping (main components and sets). */
  code: { component: string; import: string; source: string; props: Record<string, Record<string, string>>; text: Record<string, string> } | null;
}

export type Constraint = "min" | "max" | "stretch" | "center" | "scale";

export interface LayerInfo {
  id: number;
  name: string;
  kind: NodeKind;
  depth: number;
  parent: number;
  children: number;
  visible: boolean;
  locked: boolean;
  component: "main" | "instance" | null;
  /** Part of a component or instance (shown in purple). */
  purple: boolean;
}

export interface ComponentInfo {
  id: number;
  name: string;
  w: number;
  h: number;
  instances: number;
}

export interface Label {
  id: number;
  name: string;
  component: boolean;
  x: number;
  y: number;
  w: number;
  h: number;
  width: number;
  height: number;
  ready: boolean;
  section: boolean;
}

export interface ViewState {
  zoom: number;
  /** World point at the canvas's top-left corner. */
  x: number;
  y: number;
  /** Rotation of the single selected layer. */
  rotation: number;
  frames: Label[];
  selection: Label | null;
  undo: boolean;
  redo: boolean;
}

interface Raw {
  HEAPU8: Uint8Array;
  UTF8ToString(ptr: number): string;
  _pergel_init(width: number, height: number, dpr: number): number;
  _pergel_load(which: number, count: number): void;
  _pergel_fit(): void;
  _pergel_view_area(): number;
  _pergel_view_show(x: number, y: number, w: number, h: number): void;
  _pergel_fit_selection(): void;
  _pergel_resize(width: number, height: number, dpr: number): number;
  _pergel_pan(dx: number, dy: number): void;
  _pergel_zoom(factor: number, x: number, y: number): void;
  _pergel_zoom_level(): number;
  _pergel_render(): number;
  _pergel_pick(x: number, y: number, deep: number): number;
  _pergel_dive(x: number, y: number): number;
  _pergel_set_hover(id: number): void;
  _pergel_select(id: number, additive: number): void;
  _pergel_is_selected(id: number): number;
  _pergel_select_relative(which: number): void;
  _pergel_set_marquee(x: number, y: number, w: number, h: number): void;
  _pergel_select_area(x: number, y: number, w: number, h: number, additive: number): void;
  _pergel_begin(): void;
  _pergel_commit(): number;
  _pergel_abort(): number;
  _pergel_panics(): number;
  _pergel_last_panic(): number;
  _pergel_doc_local_change(): number;
  _pergel_doc_apply_patch(): number;
  _pergel_doc_revert(): number;
  _pergel_doc_merge_live(): number;
  _pergel_undo(): number;
  _pergel_redo(): number;
  _pergel_move_selection(dx: number, dy: number): void;
  _pergel_drop(x: number, y: number): void;
  _pergel_nudge(dx: number, dy: number): void;
  _pergel_create(kind: number, x: number, y: number, w: number, h: number): number;
  _pergel_delete_selection(): number;
  _pergel_group(kind: number): number;
  _pergel_ungroup(): number;
  _pergel_duplicate(): number;
  _pergel_copy(): number;
  _pergel_paste(): number;
  _pergel_pages_json(): number;
  _pergel_set_reactions(id: number): number;
  _pergel_show_connections(on: number): void;
  _pergel_show_grids(on: number): void;
  _pergel_guides_json(): number;
  _pergel_guides_set(): number;
  _pergel_screens_json(): number;
  _pergel_hotspots_json(screen: number): number;
  _pergel_screen_of(id: number): number;
  _pergel_variables_json(): number;
  _pergel_styles_json(): number;
  _pergel_styles_set(): number;
  _pergel_style_link(): number;
  _pergel_style_create(): number;
  _pergel_variables_set(): number;
  _pergel_bind(): number;
  _pergel_boolean(op: number): number;
  _pergel_find_replace(): number;
  _pergel_join(): number;
  _pergel_comments_json(): number;
  _pergel_comment_op(): number;
  _pergel_text_on_path(): number;
  _pergel_offset_path(d: number, join: number): number;
  _pergel_simplify(tolerance: number): number;
  _pergel_scale(k: number, anchor: number): void;
  _pergel_scroll_capture(id: number, max: number): number;
  _pergel_present_reset(): void;
  _pergel_present_vars(): number;
  _pergel_present_set_var(): void;
  _pergel_present_swap(instance: number, variant: number): void;
  _pergel_present_capture(screen: number, scale: number): number;
  _pergel_present_info(): number;
  _pergel_variants_of(id: number): number;
  _pergel_scroll_area(id: number): number;
  _pergel_select_inverse(): number;
  _pergel_round_to_pixel(): number;
  _pergel_next_frame(dir: number): number;
  _pergel_move_start(): void;
  _pergel_move_total(dx: number, dy: number, snap: number, axis: number): void;
  _pergel_move_end(): void;
  _pergel_combine_variants(): number;
  _pergel_set_variant(): number;
  _pergel_vector_begin(id: number): number;
  _pergel_vector_end(): void;
  _pergel_vector_state(): number;
  _pergel_vector_down(x: number, y: number, shift: number, alt: number): number;
  _pergel_vector_move(x: number, y: number): void;
  _pergel_vector_up(): void;
  _pergel_vector_delete(): number;
  _pergel_pen_down(x: number, y: number): number;
  _pergel_pencil_begin(x: number, y: number): void;
  _pergel_pencil_point(x: number, y: number): void;
  _pergel_pencil_end(): void;
  _pergel_select_matching(): number;
  _pergel_use_as_mask(): number;
  _pergel_outline_stroke(): number;
  _pergel_figma_document(): number;
  _pergel_page_switch(i: number): number;
  _pergel_page_add(): number;
  _pergel_page_rename(i: number): number;
  _pergel_page_delete(i: number): number;
  _pergel_page_duplicate(i: number): number;
  _pergel_page_move(from: number, to: number): number;
  _pergel_import_svg(x: number, y: number, nameLen: number): number;
  _pergel_place_image(x: number, y: number, nameLen: number): number;
  _pergel_set_image(): number;
  _pergel_image_bytes(): number;
  _pergel_reorder(op: number): void;
  _pergel_move_to(target: number, place: number): number;
  _pergel_set_flag(id: number, flag: number, value: number): void;
  _pergel_patch(id: number): number;
  _pergel_auto_layout(add: number): number;
  _pergel_create_component(): number;
  _pergel_create_instance(main: number, x: number, y: number): number;
  _pergel_detach_instance(): number;
  _pergel_reset_instance(): void;
  _pergel_go_to_main(): number;
  _pergel_components_json(): number;
  _pergel_thumbnail(id: number, max: number): number;
  _pergel_agent(): number;
  _pergel_doc_open(slot: number): number;
  _pergel_layers_at(x: number, y: number): number;
  _pergel_pdf(id: number): number;
  _pergel_pdf_pages(): number;
  _pergel_selection_colors(): number;
  _pergel_replace_color(): number;
  _pergel_svg(id: number): number;
  _pergel_text_content(): number;
  _pergel_paste_replace(): number;
  _pergel_resize_to_fit(): number;
  _pergel_doc_switch(slot: number): void;
  _pergel_doc_close(slot: number): void;
  _pergel_doc_json(): number;
  _pergel_doc_revision(): number;
  _pergel_doc_replace(): number;
  _pergel_doc_json_live(): number;
  _pergel_doc_merge(): number;
  _pergel_branch_json(): number;
  _pergel_library_package(): number;
  _pergel_library_refs(): number;
  _pergel_library_use(): number;
  _pergel_library_update(): number;
  _pergel_branch_set(): void;
  _pergel_doc_diff(): number;
  _pergel_handle_at(x: number, y: number): number;
  _pergel_transform_begin(handle: number, x: number, y: number): number;
  _pergel_transform_to(x: number, y: number, flags: number): void;
  _pergel_transform_end(): void;
  _pergel_align(op: number): void;
  _pergel_flip(horizontal: number): void;
  _pergel_rotate_by(degrees: number): void;
  _pergel_swap_fill_stroke(): void;
  _pergel_text_buffer(len: number): number;
  _pergel_add_font(nameLen: number): number;
  _pergel_fonts_json(): number;
  _pergel_used_fonts(): number;
  _pergel_text_style_range(id: number, start: number, end: number, toggle: number): number;
  _pergel_create_text(x: number, y: number, w: number, h: number): number;
  _pergel_set_text(id: number): number;
  _pergel_set_text_style(id: number): number;
  _pergel_set_editing(id: number): void;
  _pergel_rename(id: number): number;
  _pergel_selection_json(): number;
  _pergel_layers_json(): number;
  _pergel_view_json(): number;
}

type Factory = (options: Record<string, unknown>) => Promise<Raw>;

export const CREATE_KIND = { frame: 0, rectangle: 1, ellipse: 2, line: 3, arrow: 4, polygon: 5, star: 6, section: 7, slice: 8 } as const;

/** Thumbnail id meaning "the whole page". */
export const PAGE = 0xffffffff;

export const RELATIVE = { parent: 0, children: 1, next: 2, previous: 3, siblings: 4 } as const;
export const ORDER = { forward: 0, backward: 1, front: 2, back: 3 } as const;
export const FLAG = { visible: 0, locked: 1, clip: 2 } as const;
export const ALIGN = {
  left: 0,
  centerX: 1,
  right: 2,
  top: 3,
  centerY: 4,
  bottom: 5,
  distributeX: 6,
  distributeY: 7,
  tidy: 8,
  pack: 9,
} as const;

export const INIT_ERRORS: Record<number, string> = {
  [-1]: "WebGL2 context could not be created",
  [-2]: "Skia could not load the GL interface",
  [-3]: "Skia GPU context could not be created",
  [-4]: "Render surface could not be created",
};

export class Engine {
  private constructor(private raw: Raw) {}

  static async load(canvas: HTMLCanvasElement): Promise<Engine> {
    const url = new URL("/engine/pergel-wasm.js", location.origin).href;
    const { default: create } = (await import(/* @vite-ignore */ url)) as { default: Factory };
    return new Engine(await create({ canvas }));
  }

  private json<T>(ptr: number): T {
    return JSON.parse(this.raw.UTF8ToString(ptr)) as T;
  }

  init(width: number, height: number, dpr: number) {
    return this.raw._pergel_init(width, height, dpr);
  }
  loadSample() {
    this.raw._pergel_load(0, 0);
  }
  loadBenchmark(count: number) {
    this.raw._pergel_load(1, count);
  }
  /** The part of the canvas in view, in world coordinates. */
  viewArea() {
    return this.json<[number, number, number, number] | null>(this.raw._pergel_view_area());
  }
  /** Shows exactly this area of the canvas, as large as it fits. */
  viewShow(x: number, y: number, w: number, h: number) {
    this.raw._pergel_view_show(x, y, w, h);
  }
  fit() {
    this.raw._pergel_fit();
  }
  fitSelection() {
    this.raw._pergel_fit_selection();
  }
  resize(width: number, height: number, dpr: number) {
    return this.raw._pergel_resize(width, height, dpr);
  }
  pan(dx: number, dy: number) {
    this.raw._pergel_pan(dx, dy);
  }
  zoom(factor: number, x: number, y: number) {
    this.raw._pergel_zoom(factor, x, y);
  }
  zoomLevel() {
    return this.raw._pergel_zoom_level();
  }
  render() {
    return this.raw._pergel_render();
  }
  pick(x: number, y: number, deep = false) {
    return this.raw._pergel_pick(x, y, deep ? 1 : 0);
  }
  dive(x: number, y: number) {
    return this.raw._pergel_dive(x, y);
  }
  setHover(id: number) {
    this.raw._pergel_set_hover(id);
  }
  select(id: number, additive = false) {
    this.raw._pergel_select(id, additive ? 1 : 0);
  }
  isSelected(id: number) {
    return this.raw._pergel_is_selected(id) === 1;
  }
  selectRelative(which: keyof typeof RELATIVE) {
    this.raw._pergel_select_relative(RELATIVE[which]);
  }
  setMarquee(x: number, y: number, w: number, h: number) {
    this.raw._pergel_set_marquee(x, y, w, h);
  }
  selectArea(x: number, y: number, w: number, h: number, additive = false) {
    this.raw._pergel_select_area(x, y, w, h, additive ? 1 : 0);
  }
  begin() {
    this.raw._pergel_begin();
  }
  commit() {
    return this.raw._pergel_commit() === 1;
  }
  /** How many engine calls have hit a bug (a panic, caught and rolled back). */
  panics() {
    return this.raw._pergel_panics();
  }
  /** The last such bug's message. */
  lastPanic() {
    return this.json<string>(this.raw._pergel_last_panic());
  }
  /** Throws away the innermost open step (opened with `begin`). */
  abort() {
    return this.raw._pergel_abort() === 1;
  }
  undo() {
    return this.raw._pergel_undo() === 1;
  }
  redo() {
    return this.raw._pergel_redo() === 1;
  }
  moveSelection(dx: number, dy: number) {
    this.raw._pergel_move_selection(dx, dy);
  }
  drop(x: number, y: number) {
    this.raw._pergel_drop(x, y);
  }
  nudge(dx: number, dy: number) {
    this.raw._pergel_nudge(dx, dy);
  }
  create(kind: keyof typeof CREATE_KIND, x: number, y: number, w: number, h: number) {
    return this.raw._pergel_create(CREATE_KIND[kind], x, y, w, h);
  }
  deleteSelection() {
    return this.raw._pergel_delete_selection();
  }
  group(frame = false) {
    return this.raw._pergel_group(frame ? 1 : 0);
  }
  ungroup() {
    return this.raw._pergel_ungroup();
  }
  duplicate() {
    return this.raw._pergel_duplicate();
  }
  copy() {
    return this.raw.UTF8ToString(this.raw._pergel_copy());
  }
  paste(text: string) {
    this.writeText(text);
    return this.raw._pergel_paste();
  }
  reorder(op: keyof typeof ORDER) {
    this.raw._pergel_reorder(ORDER[op]);
  }
  moveTo(target: number, place: "above" | "below" | "inside") {
    return this.raw._pergel_move_to(target, place === "above" ? 0 : place === "below" ? 1 : 2) === 1;
  }
  setFlag(id: number, flag: keyof typeof FLAG, value?: boolean) {
    this.raw._pergel_set_flag(id, FLAG[flag], value === undefined ? -1 : value ? 1 : 0);
  }
  /** Applies `patch` to `id`, or to every selected layer when `id` is -1. */
  patch(patch: Patch, id = -1) {
    this.writeText(JSON.stringify(patch));
    return this.raw._pergel_patch(id) > 0;
  }
  createComponent() {
    return this.raw._pergel_create_component();
  }
  /** An instance centred at a canvas point, or at the view centre without one. */
  createInstance(main: number, x = -1, y = -1) {
    return this.raw._pergel_create_instance(main, x, y);
  }
  detachInstance() {
    return this.raw._pergel_detach_instance();
  }
  resetInstance() {
    this.raw._pergel_reset_instance();
  }
  goToMain() {
    return this.raw._pergel_go_to_main();
  }
  components() {
    return this.json<ComponentInfo[]>(this.raw._pergel_components_json()) ?? [];
  }
  /** A one-page PDF of a layer. */
  /** One PDF with a page per layer (a slide deck). */
  pdfPages(ids: number[]): Uint8Array | null {
    this.writeText(JSON.stringify(ids));
    const ptr = this.raw._pergel_pdf_pages();
    if (!ptr) return null;
    const len = new DataView(this.raw.HEAPU8.buffer, ptr, 4).getUint32(0, true);
    return this.raw.HEAPU8.slice(ptr + 4, ptr + 4 + len);
  }
  pdf(id: number): Uint8Array | null {
    const ptr = this.raw._pergel_pdf(id);
    if (!ptr) return null;
    const heap = this.raw.HEAPU8;
    const len = new DataView(heap.buffer, ptr, 4).getUint32(0, true);
    return heap.slice(ptr + 4, ptr + 4 + len);
  }
  /** Fill and stroke colours inside the selection, most used first. */
  selectionColors() {
    return this.json<{ color: string; opacity: number; count: number }[]>(this.raw._pergel_selection_colors()) ?? [];
  }
  /** Replaces a colour in the selection's fills and strokes. */
  replaceColor(from: string, fromOpacity: number, to: string, toOpacity: number) {
    this.writeText(JSON.stringify({ from, fromOpacity, to, toOpacity }));
    return this.raw._pergel_replace_color();
  }
  /** Layers under a canvas point, topmost first ("Select layer" menu). */
  layersAt(x: number, y: number) {
    return this.json<{ id: number; name: string; kind: NodeKind; type: string }[]>(this.raw._pergel_layers_at(x, y)) ?? [];
  }
  /** SVG of a layer, or of the first selected layer. */
  svg(id = -1) {
    return this.raw.UTF8ToString(this.raw._pergel_svg(id));
  }
  /** Characters of the selected text layers. */
  textContent() {
    return this.raw.UTF8ToString(this.raw._pergel_text_content());
  }
  pasteReplace(text: string) {
    this.writeText(text);
    return this.raw._pergel_paste_replace();
  }
  resizeToFit() {
    return this.raw._pergel_resize_to_fit();
  }
  /** Opens `.pergel` JSON as tab `slot` and shows it; false when the text is not a document. */
  docOpen(slot: number, json: string) {
    this.writeText(json);
    return this.raw._pergel_doc_open(slot) === 1;
  }
  /** Shows tab `slot` (a new empty document if it was never opened). */
  docSwitch(slot: number) {
    this.raw._pergel_doc_switch(slot);
  }
  docClose(slot: number) {
    this.raw._pergel_doc_close(slot);
  }
  docJson() {
    return this.raw.UTF8ToString(this.raw._pergel_doc_json());
  }
  /** Live collaboration: show another peer's copy of the document (1), busy (0), invalid (-1). */
  docReplace(json: string) {
    this.writeText(json);
    return this.raw._pergel_doc_replace();
  }
  /** Live collaboration: the document with stable uids on its top-level layers. */
  docJsonLive(prefix: string) {
    this.writeText(prefix);
    return this.raw.UTF8ToString(this.raw._pergel_doc_json_live());
  }
  /** Team libraries: this document's components as a library package. */
  libraryPackage(prefix: string) {
    this.writeText(prefix);
    return (
      this.json<{ components: { key: string; name: string; set: boolean; page: number; nodeId: number | null; w: number; h: number; tree: unknown }[] }>(
        this.raw._pergel_library_package(),
      ) ?? { components: [] }
    );
  }
  /** Library components used in this document. */
  libraryRefs() {
    return this.json<{ lib: string; key: string; version: number; id: number; name: string }[]>(this.raw._pergel_library_refs()) ?? [];
  }
  /** Places an instance of a library component (imported on first use); its id or -1. */
  libraryUse(req: { lib: string; libName: string; key: string; version: number; tree: unknown; x?: number; y?: number }) {
    this.writeText(JSON.stringify(req));
    return this.raw._pergel_library_use();
  }
  /** Moves a library component's instances to a newer version; how many moved. */
  libraryUpdate(req: { lib: string; libName: string; key: string; version: number; tree: unknown }) {
    this.writeText(JSON.stringify(req));
    return this.raw._pergel_library_update();
  }
  /** Branching: the shown document's branch info, or null for a main file. */
  branch() {
    return this.json<BranchInfo | null>(this.raw._pergel_branch_json());
  }
  setBranch(b: BranchInfo | null) {
    this.writeText(JSON.stringify(b));
    this.raw._pergel_branch_set();
  }
  /** What changed from `base` to `other`, per top-level layer. */
  docDiff(base: string, other: string) {
    this.writeText(JSON.stringify({ base, other }));
    return this.json<{ page: string; name: string; change: "added" | "changed" | "deleted" }[]>(this.raw._pergel_doc_diff()) ?? [];
  }
  /** Live collaboration: stamps this peer's changes from `before` to `after` and returns them
   *  as a patch with old and new values (see core `merge.rs`). */
  docLocalChange(before: string, after: string, stamps: Stamps, peer: string) {
    this.writeText(JSON.stringify({ before, after, stamps, peer }));
    return this.json<{ stamps: Stamps; change: LivePatch } | null>(this.raw._pergel_doc_local_change());
  }
  /** Live collaboration: another peer's patch, applied where its stamps are later. */
  docApplyPatch(local: string, stamps: Stamps, patch: LivePatch) {
    this.writeText(JSON.stringify({ local, stamps, patch }));
    return this.json<{ doc: string; stamps: Stamps } | null>(this.raw._pergel_doc_apply_patch());
  }
  /** Live collaboration undo: reverts (or redoes) one of this peer's changes where nobody changed it since. */
  docRevert(current: string, patch: LivePatch, redo: boolean) {
    this.writeText(JSON.stringify({ current, patch, redo }));
    return this.json<{ doc: string; changed: number } | null>(this.raw._pergel_doc_revert());
  }
  /** Live collaboration: merges another peer's document, each part from the later stamp. */
  docMergeLive(local: string, localStamps: Stamps, remote: string, remoteStamps: Stamps) {
    this.writeText(JSON.stringify({ local, localStamps, remote, remoteStamps }));
    return this.json<{ doc: string; stamps: Stamps; ahead: boolean } | null>(this.raw._pergel_doc_merge_live());
  }
  /** Branches: three-way merge of documents (per top-level layer). */
  docMerge(base: string, local: string, remote: string) {
    this.writeText(JSON.stringify({ base, local, remote }));
    return this.json<{ doc: string; ahead: boolean } | null>(this.raw._pergel_doc_merge());
  }
  /** Changes whenever the shown document changes. */
  docRevision() {
    return this.raw._pergel_doc_revision();
  }
  /** Runs an MCP tool against the document: `{ ok, result }` or `{ ok: false, error }`. */
  agent(tool: string, args: unknown): { ok: boolean; result?: unknown; error?: string } {
    this.writeText(JSON.stringify({ tool, args }));
    return this.json(this.raw._pergel_agent());
  }
  /** A PNG data URL of a layer (or the whole page with `PAGE`), at most `max` px on its long side. */
  thumbnail(id: number, max = 160): string | null {
    return this.capture(id, max)?.url ?? null;
  }
  /** Renders a layer (or `PAGE`) to a PNG data URL with its pixel size. */
  /** Presentation view: forget "Change to" variant changes. */
  presentReset() {
    this.raw._pergel_present_reset();
  }
  /** Presentation view: every variable's value now (`{id: value}`). */
  presentVars() {
    return this.json<Record<string, VarValue>>(this.raw._pergel_present_vars()) ?? {};
  }
  /** Presentation view: set a variable for the rest of the run. */
  presentSetVar(id: string, value: VarValue) {
    this.writeText(JSON.stringify({ id, value }));
    this.raw._pergel_present_set_var();
  }
  /** Presentation view: show an instance as another variant (-1: its own again). */
  presentSwap(instance: number, variant: number) {
    this.raw._pergel_present_swap(instance, variant < 0 ? 0xffffffff : variant);
  }
  /** Presentation view: a screen with the variant changes applied, its content area and hotspots. */
  presentCapture(screen: number, scale = 1) {
    const shot = this.capture(screen, 0, this.raw._pergel_present_capture(screen, scale));
    const info = this.json<{ area: { x: number; y: number; w: number; h: number }; spots: (Hotspot & { instance: number | null })[] }>(this.raw._pergel_present_info());
    return shot && info ? { ...shot, ...info } : null;
  }
  /** The variants of a layer's component set: [{id, name}]. */
  variantsOf(id: number) {
    return this.json<{ id: number; name: string }[]>(this.raw._pergel_variants_of(id)) ?? [];
  }
  /** A scrolling screen's whole content (presentation view) and its area relative to the screen. */
  scrollCapture(id: number, scale = 1): { url: string; width: number; height: number; area: { x: number; y: number; w: number; h: number } } | null {
    const area = this.json<{ x: number; y: number; w: number; h: number }>(this.raw._pergel_scroll_area(id));
    const max = area ? Math.min(8192, Math.round(Math.max(area.w, area.h) * scale)) : 0;
    const shot = area && this.capture(id, max, this.raw._pergel_scroll_capture(id, max));
    return shot && area ? { ...shot, area } : null;
  }
  capture(id: number, max = 1024, rendered?: number): { url: string; width: number; height: number } | null {
    const ptr = rendered ?? this.raw._pergel_thumbnail(id, max);
    if (!ptr) return null;
    const heap = this.raw.HEAPU8;
    const head = new DataView(heap.buffer, ptr, 8);
    const w = head.getUint32(0, true);
    const h = head.getUint32(4, true);
    const pixels = new Uint8ClampedArray(w * h * 4);
    pixels.set(heap.subarray(ptr + 8, ptr + 8 + w * h * 4));
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    canvas.getContext("2d")?.putImageData(new ImageData(pixels, w, h), 0, 0);
    return { url: canvas.toDataURL("image/png"), width: w, height: h };
  }
  /** Shift+A adds auto layout (wrapping the selection if needed); `false` removes it. */
  autoLayout(add: boolean) {
    return this.raw._pergel_auto_layout(add ? 1 : 0) === 1;
  }
  /** Selection handle under a screen point: 0–7 resize (clockwise from top-left), 8–11 rotate, -1 none. */
  handleAt(x: number, y: number) {
    return this.raw._pergel_handle_at(x, y);
  }
  transformBegin(handle: number, x: number, y: number) {
    return this.raw._pergel_transform_begin(handle, x, y) === 1;
  }
  transformTo(x: number, y: number, shift: boolean, alt: boolean, scale = false) {
    this.raw._pergel_transform_to(x, y, (shift ? 1 : 0) | (alt ? 2 : 0) | (scale ? 4 : 0));
  }
  /** Scale tool: scale the selection and its contents by `k` about an anchor (0–8, 4 = centre). */
  scale(k: number, anchor: number) {
    this.raw._pergel_scale(k, anchor);
  }
  transformEnd() {
    this.raw._pergel_transform_end();
  }
  align(op: keyof typeof ALIGN) {
    this.raw._pergel_align(ALIGN[op]);
  }
  flip(horizontal: boolean) {
    this.raw._pergel_flip(horizontal ? 1 : 0);
  }
  rotateBy(degrees: number) {
    this.raw._pergel_rotate_by(degrees);
  }
  swapFillStroke() {
    this.raw._pergel_swap_fill_stroke();
  }
  private writeText(text: string) {
    this.writeBytes(new TextEncoder().encode(text));
  }
  private writeBytes(...parts: Uint8Array[]) {
    const len = parts.reduce((n, p) => n + p.length, 0);
    let ptr = this.raw._pergel_text_buffer(len);
    for (const p of parts) {
      this.raw.HEAPU8.set(p, ptr);
      ptr += p.length;
    }
  }
  /** Converts a Figma REST file (+ its image fills as base64 by imageRef) to `.pergel` JSON. */
  figmaDocument(file: unknown, images: Record<string, string>) {
    this.writeText(JSON.stringify({ file, images }));
    return this.json<{ document?: unknown; report?: FigmaReport; error?: string }>(this.raw._pergel_figma_document()) ?? { error: "No answer" };
  }
  /** Union, subtract, intersect, exclude or flatten the selection into one vector layer. */
  boolean(op: "union" | "subtract" | "intersect" | "exclude" | "flatten") {
    return this.raw._pergel_boolean({ union: 0, subtract: 1, intersect: 2, exclude: 3, flatten: 4 }[op]);
  }
  /** Vector edit mode on a layer (-1: the selection). Shapes become vectors. */
  vectorBegin(id = -1) {
    return this.raw._pergel_vector_begin(id) === 1;
  }
  vectorEnd() {
    this.raw._pergel_vector_end();
  }
  /** 0 not editing, 1 editing, 2 drawing with the pen. */
  vectorState() {
    return this.raw._pergel_vector_state();
  }
  /** 1 anchor, 2 handle, 3 anchor added on a segment, 4 empty space. */
  vectorDown(x: number, y: number, shift: boolean, alt: boolean) {
    return this.raw._pergel_vector_down(x, y, shift ? 1 : 0, alt ? 1 : 0);
  }
  vectorMove(x: number, y: number) {
    this.raw._pergel_vector_move(x, y);
  }
  vectorUp() {
    this.raw._pergel_vector_up();
  }
  vectorDelete() {
    return this.raw._pergel_vector_delete() === 1;
  }
  /** Pencil: a freehand stroke. */
  pencilBegin(x: number, y: number) {
    this.raw._pergel_pencil_begin(x, y);
  }
  pencilPoint(x: number, y: number) {
    this.raw._pergel_pencil_point(x, y);
  }
  pencilEnd() {
    this.raw._pergel_pencil_end();
  }
  /** 1 anchor added, 2 path closed. */
  penDown(x: number, y: number) {
    return this.raw._pergel_pen_down(x, y);
  }
  /** Combine as variants: the selected main components become a component set. */
  combineVariants() {
    return this.raw._pergel_combine_variants();
  }
  /** Switches the selected instances' variant property. */
  setVariant(property: string, value: string) {
    this.writeText(JSON.stringify({ property, value }));
    return this.raw._pergel_set_variant();
  }
  /** Dragging with smart guides: start, move by the total offset from the start, end. */
  moveStart() {
    this.raw._pergel_move_start();
  }
  moveTotal(dx: number, dy: number, snap: boolean, lockAxis: boolean) {
    this.raw._pergel_move_total(dx, dy, snap ? 1 : 0, lockAxis ? 1 : 0);
  }
  moveEnd() {
    this.raw._pergel_move_end();
  }
  /** Find (dryRun selects the matches) and replace in text layers. */
  findReplace(find: string, replace: string, matchCase: boolean, dryRun: boolean) {
    this.writeText(JSON.stringify({ find, replace, matchCase, dryRun }));
    return this.json<{ layers: number; matches: number }>(this.raw._pergel_find_replace()) ?? { layers: 0, matches: 0 };
  }
  selectInverse() {
    return this.raw._pergel_select_inverse();
  }
  roundToPixel() {
    return this.raw._pergel_round_to_pixel();
  }
  /** Selects and zooms to the next (1) or previous (-1) top-level frame. */
  nextFrame(dir: 1 | -1) {
    return this.raw._pergel_next_frame(dir);
  }
  /** Selects every layer that matches the selected one. */
  selectMatching() {
    return this.raw._pergel_select_matching();
  }
  /** Use as mask (toggle). */
  useAsMask() {
    return this.raw._pergel_use_as_mask() === 1;
  }
  /** Turns selected strokes into filled outlines. */
  /** Offset path: new layers `d` px outside (negative: inside); join 0 miter, 1 round, 2 bevel. */
  offsetPath(d: number, join: number) {
    return this.raw._pergel_offset_path(d, join);
  }
  /** Simplify vertices of the selected vectors. */
  simplify(tolerance: number) {
    return this.raw._pergel_simplify(tolerance);
  }
  /** Type on path (text + vector selected); 1 attached, 2 detached, 0 nothing to do. */
  textOnPath() {
    return this.raw._pergel_text_on_path();
  }
  /** Comment threads on the shown page. */
  comments() {
    return this.json<CommentThread[]>(this.raw._pergel_comments_json()) ?? [];
  }
  /** add / reply / resolve / delete a comment thread; returns its id ("" when nothing changed). */
  commentOp(op: Record<string, unknown>) {
    this.writeText(JSON.stringify(op));
    return this.raw.UTF8ToString(this.raw._pergel_comment_op()).replace(/^"|"$/g, "");
  }
  /** Join (Ctrl+J) the selected vectors' open paths. */
  join() {
    return this.raw._pergel_join();
  }
  outlineStroke() {
    return this.raw._pergel_outline_stroke();
  }
  styles(): Styles {
    return this.json<Styles>(this.raw._pergel_styles_json()) ?? { list: [] };
  }
  setStyles(s: Styles) {
    this.writeText(JSON.stringify(s));
    return this.raw._pergel_styles_set();
  }
  /** Links the selection to a style (null unlinks). */
  linkStyle(kind: StyleKind, style: string | null) {
    this.writeText(JSON.stringify({ kind, style }));
    return this.raw._pergel_style_link() > 0;
  }
  /** Creates a style from the first selected layer and links the selection. */
  createStyle(kind: StyleKind, name: string): string | null {
    this.writeText(JSON.stringify({ kind, name }));
    return this.json<string>(this.raw._pergel_style_create());
  }
  variables(): Variables {
    return this.json<Variables>(this.raw._pergel_variables_json()) ?? { collections: [], variables: [] };
  }
  /** Replaces the document's variables; bound layers follow. */
  setVariables(v: Variables) {
    this.writeText(JSON.stringify(v));
    return this.raw._pergel_variables_set();
  }
  /** Binds a property of the selection to a variable (null unbinds). */
  bind(prop: BindProp, variable: string | null) {
    this.writeText(JSON.stringify({ prop, variable }));
    return this.raw._pergel_bind() > 0;
  }
  /** Replaces a layer's (or with -1 the selection's) prototype interactions. */
  setReactions(id: number, list: Reaction[]) {
    this.writeText(JSON.stringify(list));
    return this.raw._pergel_set_reactions(id) > 0;
  }
  /** The shown page's ruler guides. */
  guides() {
    return this.json<{ vertical: boolean; at: number }[]>(this.raw._pergel_guides_json()) ?? [];
  }
  setGuides(list: { vertical: boolean; at: number }[]) {
    this.writeText(JSON.stringify(list));
    this.raw._pergel_guides_set();
  }
  showGrids(on: boolean) {
    this.raw._pergel_show_grids(on ? 1 : 0);
  }
  showConnections(on: boolean) {
    this.raw._pergel_show_connections(on ? 1 : 0);
  }
  /** Top-level frames of the page. */
  screens() {
    return this.json<{ id: number; name: string; w: number; h: number; scroll: Scroll; fill: string }[]>(this.raw._pergel_screens_json()) ?? [];
  }
  hotspots(screen: number) {
    return this.json<Hotspot[]>(this.raw._pergel_hotspots_json(screen)) ?? [];
  }
  /** The top-level frame a layer is in, or -1. */
  screenOf(id: number) {
    return this.raw._pergel_screen_of(id);
  }
  /** The document's pages in order. */
  pages() {
    return this.json<{ name: string; current: boolean }[]>(this.raw._pergel_pages_json()) ?? [];
  }
  pageSwitch(i: number) {
    return this.raw._pergel_page_switch(i) === 1;
  }
  /** Adds a page after the current one and shows it. */
  pageAdd() {
    return this.raw._pergel_page_add();
  }
  pageRename(i: number, name: string) {
    this.writeText(name);
    return this.raw._pergel_page_rename(i) === 1;
  }
  pageDelete(i: number) {
    return this.raw._pergel_page_delete(i) === 1;
  }
  pageDuplicate(i: number) {
    return this.raw._pergel_page_duplicate(i);
  }
  pageMove(from: number, to: number) {
    return this.raw._pergel_page_move(from, to) === 1;
  }
  /** Imports SVG markup as vector layers centred on a canvas point (NaN: the view's centre). */
  importSvg(name: string, svg: string, x = NaN, y = NaN) {
    const n = new TextEncoder().encode(name);
    this.writeBytes(n, new TextEncoder().encode(svg));
    return this.raw._pergel_import_svg(x, y, n.length);
  }
  /** Places an image file as a layer with an image fill; -1 when it is not an image. */
  placeImage(name: string, bytes: Uint8Array, x = NaN, y = NaN) {
    const n = new TextEncoder().encode(name);
    this.writeBytes(n, bytes);
    return this.raw._pergel_place_image(x, y, n.length);
  }
  /** Puts an image into the selected layers' fills. */
  setImage(bytes: Uint8Array) {
    this.writeBytes(bytes);
    return this.raw._pergel_set_image();
  }
  /** The original file behind an image fill. */
  imageBytes(hash: string): Uint8Array | null {
    this.writeText(hash);
    const ptr = this.raw._pergel_image_bytes();
    if (!ptr) return null;
    const heap = this.raw.HEAPU8;
    const len = new DataView(heap.buffer, ptr, 4).getUint32(0, true);
    return len ? heap.slice(ptr + 4, ptr + 4 + len) : null;
  }
  /** Registers a font file; `family` overrides the name stored in the font. */
  addFont(bytes: Uint8Array, family = "") {
    const name = new TextEncoder().encode(family);
    this.writeBytes(name, bytes);
    return this.raw._pergel_add_font(name.length) === 1;
  }
  fonts() {
    return this.json<FontFamily[]>(this.raw._pergel_fonts_json()) ?? [];
  }
  /** Styles a character range of a text layer (mixed styles); `toggle` flips bold/italic/underline. */
  styleTextRange(id: number, start: number, end: number, style: Record<string, unknown>, toggle = false) {
    this.writeText(JSON.stringify(style));
    return this.raw._pergel_text_style_range(id, start, end, toggle ? 1 : 0) === 1;
  }
  /** Font families the document uses, and whether each is loaded. */
  usedFonts() {
    return this.json<{ family: string; weights: number[]; italic: boolean; available: boolean }[]>(this.raw._pergel_used_fonts()) ?? [];
  }
  createText(x: number, y: number, w: number, h: number) {
    return this.raw._pergel_create_text(x, y, w, h);
  }
  setText(id: number, content: string) {
    this.writeText(content);
    return this.raw._pergel_set_text(id) === 1;
  }
  /** Applies typography to `id`, or to every selected text layer when `id` is -1. */
  setTextStyle(id: number, style: TextStyle) {
    this.writeText(JSON.stringify(style));
    return this.raw._pergel_set_text_style(id) > 0;
  }
  setEditing(id: number) {
    this.raw._pergel_set_editing(id);
  }
  rename(id: number, name: string) {
    this.writeText(name);
    return this.raw._pergel_rename(id) === 1;
  }
  selection() {
    return this.json<NodeInfo[]>(this.raw._pergel_selection_json()) ?? [];
  }
  layers() {
    return this.json<LayerInfo[]>(this.raw._pergel_layers_json()) ?? [];
  }
  view() {
    return this.json<ViewState>(this.raw._pergel_view_json());
  }
}
