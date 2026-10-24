// The arrangement's WebGL2 renderer, on the ADR 0009 spike's proven design (one instanced draw for every clip, the peak pyramid read from a texture in
// the fragment shader). Four instanced programs:
//
// - rects: lanes, grid lines, the loop band, section lines, row lines (under the clips) and the
//   drop indicator and rubber band (over them), rebuilt every frame on the CPU (a few hundred);
// - clips: rounded bodies, headers and outlines (scene.ts, CLIP_FLOATS);
// - notes: mini notes and loop lines (NOTE_FLOATS);
// - waves: the waveform envelope of each piece of an audio clip, its fades, and the dashed
//   centre line while its peaks are computed (WAVE_FLOATS, WAVE_INTS).
//
// The waveform shader is the prototype's `PeakPyramid.envelope` per device pixel (src/lib/
// peaks.ts `envelope`): the coarsest level whose bins are at most a pixel's frames, the bins
// from floor(start / size) to max(lo + 1, floor((start + fpp) / size)), 0 outside the media.
// Scrolling and zooming only change uniforms; the instance buffers upload when the scene does.

import type { PeaksAtlas } from "./atlas";
import { parseHex, THEME, unit } from "./colors";
import { CLIP_FLOATS, NOTE_FLOATS, type SceneLayer, WAVE_FLOATS, WAVE_INTS } from "./scene";

/** Floats per rect instance: x, y, width, height (CSS pixels), then r, g, b, a (0–1). */
export const RECT_FLOATS = 8;

/** The texels per row of the peaks atlas. */
export const ATLAS_WIDTH = 4096;

const HEADER = `#version 300 es
precision highp float;
precision highp int;
uniform vec4 u_view;  // scrollX, pixels per beat (CSS), scrollY, device pixels per CSS pixel
uniform vec2 u_size;  // the canvas in device pixels
vec4 place(vec2 css) {
  vec2 ndc = css * u_view.w / u_size * 2.0 - 1.0;
  return vec4(ndc.x, -ndc.y, 0.0, 1.0);
}
float beatX(float beat) { return beat * u_view.y - u_view.x; }
vec2 corner() { return vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)); }
`;

const FRAGMENT_HEADER = `#version 300 es
precision highp float;
precision highp int;
uniform vec4 u_view;
uniform vec2 u_size;
out vec4 o_color;
// The fragment's centre in CSS pixels from the canvas's top left.
vec2 cssPoint() { return vec2(gl_FragCoord.x, u_size.y - gl_FragCoord.y) / u_view.w; }
`;

const RECT_VS = `${HEADER}
layout(location = 0) in vec4 a_rect;
layout(location = 1) in vec4 a_color;
flat out vec4 v_color;
void main() {
  v_color = a_color;
  gl_Position = place(a_rect.xy + corner() * a_rect.zw);
}`;

const RECT_FS = `${FRAGMENT_HEADER}
flat in vec4 v_color;
void main() { o_color = vec4(v_color.rgb * v_color.a, v_color.a); }`;

// The clip's rectangle, as hit.ts's clipRect: rounded beats, half a pixel in, 1.5 below the
// row's top, three pixels short of the row.
const CLIP_RECT = `
vec4 clipRect(float start, float end, float top, float height) {
  float x0 = floor(beatX(start) + 0.5);
  float x1 = floor(beatX(end) + 0.5);
  return vec4(x0 + 0.5, top - u_view.z + 1.5, max(2.0, x1 - x0 - 1.0), height - 3.0);
}
// The content under the 14 px header, shown on rects at least 26 px high.
vec4 contentArea(vec4 rect) {
  return rect.w >= 26.0 ? vec4(rect.x, rect.y + 14.0, rect.z, rect.w - 14.0) : rect;
}`;

const CLIP_VS = `${HEADER}${CLIP_RECT}
layout(location = 0) in vec4 a_span;     // start, end, top, height
layout(location = 1) in vec3 a_body;
layout(location = 2) in vec3 a_header;
layout(location = 3) in vec3 a_outline;
layout(location = 4) in vec2 a_style;    // opacity, outline width
out vec2 v_local;
flat out vec2 v_dims;
flat out vec3 v_body;
flat out vec3 v_header;
flat out vec3 v_outline;
flat out vec2 v_style;
void main() {
  vec4 rect = clipRect(a_span.x, a_span.y, a_span.z, a_span.w);
  // One device pixel of margin for the edge's antialiasing.
  float margin = 1.0 / u_view.w;
  vec2 local = corner() * (rect.zw + 2.0 * margin) - margin;
  v_local = local;
  v_dims = rect.zw;
  v_body = a_body;
  v_header = a_header;
  v_outline = a_outline;
  v_style = a_style;
  gl_Position = place(rect.xy + local);
}`;

const CLIP_FS = `${FRAGMENT_HEADER}
in vec2 v_local;
flat in vec2 v_dims;
flat in vec3 v_body;
flat in vec3 v_header;
flat in vec3 v_outline;
flat in vec2 v_style;
float roundBox(vec2 p, vec2 b, float r) {
  vec2 q = abs(p) - b + r;
  return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}
void main() {
  vec2 half_ = v_dims * 0.5;
  float radius = min(3.0, min(half_.x, half_.y));
  float d = roundBox(v_local - half_, half_, radius);
  float coverage = clamp(0.5 - d * u_view.w, 0.0, 1.0);
  if (coverage <= 0.0) discard;
  vec3 color = (v_dims.y >= 26.0 && v_local.y < 14.0) ? v_header : v_body;
  if (d > -v_style.y) color = v_outline;
  float alpha = coverage * v_style.x;
  o_color = vec4(color * alpha, alpha);
}`;

const NOTE_VS = `${HEADER}${CLIP_RECT}
layout(location = 0) in vec4 a_note;     // start, end, top, height
layout(location = 1) in vec4 a_clip;     // the clip's drawn start and end, slot, slots
layout(location = 2) in vec4 a_color;    // rgb, opacity
layout(location = 3) in float a_kind;    // 0 a note, 1 a loop line
flat out vec4 v_color;
flat out float v_kind;
out float v_y;
void main() {
  vec4 rect = clipRect(a_clip.x, a_clip.y, a_note.z, a_note.w);
  vec4 area = contentArea(rect);
  v_color = a_color;
  v_kind = a_kind;
  vec2 c = corner();
  if (area.w < 4.0) {
    gl_Position = vec4(2.0, 2.0, 0.0, 1.0);
    return;
  }
  vec2 p0;
  vec2 p1;
  if (a_kind > 0.5) {
    float x = floor(beatX(a_note.x) + 0.5);
    p0 = vec2(x, area.y);
    p1 = vec2(x + 1.0, area.y + area.w);
  } else {
    float span = a_clip.w;
    float usable = area.w - 4.0;
    float noteH = max(1.0, min(5.0, usable / span));
    float top = area.y + 2.0 + (usable - noteH * span) * 0.5 + a_clip.z * noteH;
    float x = beatX(a_note.x);
    float w = max(1.0, (a_note.y - a_note.x) * u_view.y - (u_view.y > 40.0 ? 1.0 : 0.0));
    p0 = vec2(x, top);
    p1 = vec2(x + w, top + noteH);
  }
  // Inside the clip.
  p0.x = clamp(p0.x, rect.x, rect.x + rect.z);
  p1.x = clamp(p1.x, rect.x, rect.x + rect.z);
  vec2 p = mix(p0, p1, c);
  v_y = p.y - area.y;
  gl_Position = place(p);
}`;

const NOTE_FS = `${FRAGMENT_HEADER}
flat in vec4 v_color;
flat in float v_kind;
in float v_y;
void main() {
  // A loop line is dotted: one pixel on, two off.
  if (v_kind > 0.5 && mod(floor(v_y), 3.0) >= 1.0) discard;
  o_color = vec4(v_color.rgb * v_color.a, v_color.a);
}`;

const WAVE_VS = `${HEADER}${CLIP_RECT}
layout(location = 0) in vec4 a_piece;    // the piece's start and end, the clip's drawn start and end
layout(location = 1) in vec4 a_row;      // row top, row height, frame fraction, frames per beat
layout(location = 2) in vec4 a_shape;    // gain, fade-in end, fade-out start, clip start
layout(location = 3) in vec4 a_color;    // the clip's end, rgb
layout(location = 4) in vec2 a_alpha;    // waveform alpha, opacity
layout(location = 5) in uvec4 a_asset;   // atlas offset, level-0 bins, levels, level-0 frames
layout(location = 6) in uint a_whole;    // the whole source frame at the piece's start
flat out vec4 v_area;
flat out vec4 v_piece;
flat out vec4 v_shape;
flat out vec4 v_color;
flat out vec2 v_alpha;
flat out uvec4 v_asset;
flat out uint v_whole;
flat out float v_rectX;
void main() {
  vec4 rect = clipRect(a_piece.z, a_piece.w, a_row.x, a_row.y);
  vec4 area = contentArea(rect);
  v_area = area;
  v_piece = vec4(a_piece.x, a_piece.y, a_row.z, a_row.w);
  v_shape = a_shape;
  v_color = a_color;
  v_alpha = a_alpha;
  v_asset = a_asset;
  v_whole = a_whole;
  v_rectX = rect.x;
  if (area.w < 4.0) {
    gl_Position = vec4(2.0, 2.0, 0.0, 1.0);
    return;
  }
  float x0 = max(beatX(a_piece.x), rect.x);
  float x1 = min(beatX(a_piece.y), rect.x + rect.z);
  vec2 c = corner();
  gl_Position = place(vec2(mix(x0, max(x0, x1), c.x), area.y + c.y * area.w));
}`;

const WAVE_FS = `${FRAGMENT_HEADER}
uniform highp sampler2D u_peaks;   // RG32F: min, max
flat in vec4 v_area;
flat in vec4 v_piece;
flat in vec4 v_shape;
flat in vec4 v_color;
flat in vec2 v_alpha;
flat in uvec4 v_asset;
flat in uint v_whole;
flat in float v_rectX;
const uint ATLAS_WIDTH = ${ATLAS_WIDTH}u;
vec4 over(vec4 top, vec4 bottom) { return top + bottom * (1.0 - top.a); }
vec2 bin(uint i) {
  return texelFetch(u_peaks, ivec2(int(i % ATLAS_WIDTH), int(i / ATLAS_WIDTH)), 0).rg;
}
void main() {
  vec2 p = cssPoint();
  float areaTop = v_area.y;
  float areaH = v_area.w;
  float mid = areaTop + areaH * 0.5;
  vec4 color = vec4(0.0);
  uint levels = v_asset.z;
  if (levels == 0u) {
    // Peaks being computed: a dashed centre line (dash 4, gap 2).
    if (abs(p.y - mid) <= 0.5 && mod(p.x - v_rectX, 6.0) < 4.0) color = vec4(v_color.yzw, 1.0);
  } else {
    // The device column's left edge, its first frame and its frames (envelope in peaks.ts).
    float scale = u_view.w;
    float left = floor(gl_FragCoord.x) / scale;
    float beat = (left + u_view.x) / u_view.y;
    float framesPerBeat = v_piece.w;
    float fpp = max(framesPerBeat / (u_view.y * scale), 1.0);
    float local = max(0.0, (beat - v_piece.x) * framesPerBeat + v_piece.z);
    uint size = v_asset.w;
    uint offset = v_asset.x;
    uint bins = v_asset.y;
    uint level = 0u;
    while (level + 1u < levels && float(size * 4u) <= fpp) {
      offset += bins;
      bins = (bins + 3u) / 4u;
      size *= 4u;
      level += 1u;
    }
    float whole = floor(local);
    uint start = v_whole + uint(whole);
    uint lo = start / size;
    uint hi = max(lo + 1u, (start + uint(floor(local - whole + fpp))) / size);
    float low = 0.0;
    float high = 0.0;
    if (lo < bins) {
      lo = min(lo, bins - 1u);
      hi = min(hi, bins);
      low = 1e9;
      high = -1e9;
      for (uint b = lo; b < hi && b < lo + 64u; b++) {
        vec2 t = bin(offset + b);
        low = min(low, t.r);
        high = max(high, t.g);
      }
    }
    float gain = v_shape.x;
    float half_ = areaH * 0.5 - 1.0;
    float top = mid - clamp(high * gain, -1.0, 1.0) * half_;
    float bottom = max(mid - clamp(low * gain, -1.0, 1.0) * half_, top + 1.0);
    if (p.y >= top && p.y <= bottom) color = vec4(v_color.yzw * v_alpha.x, v_alpha.x);
    // The fades: a shaded triangle and a line from the silent corner.
    float startX = v_shape.w * u_view.y - u_view.x;
    float endX = v_color.x * u_view.y - u_view.x;
    float t = (p.y - areaTop) / areaH;
    if (v_shape.y > -1e8) {
      float x1 = v_shape.y * u_view.y - u_view.x;
      float f = (p.x - startX) / max(x1 - startX, 1e-6);
      if (p.x >= startX && p.x <= x1) {
        if (t < 1.0 - f) color = over(vec4(0.0, 0.0, 0.0, 70.0 / 255.0), color);
        vec2 a = vec2(startX, areaTop + areaH);
        vec2 d = vec2(x1, areaTop) - a;
        float dist = abs(d.x * (p.y - a.y) - d.y * (p.x - a.x)) / max(length(d), 1e-6);
        if (dist <= 0.5) color = over(vec4(vec3(150.0 / 255.0), 150.0 / 255.0), color);
      }
    }
    if (v_shape.z > -1e8) {
      float x0 = v_shape.z * u_view.y - u_view.x;
      float f = (p.x - x0) / max(endX - x0, 1e-6);
      if (p.x >= x0 && p.x <= endX) {
        if (t < f) color = over(vec4(0.0, 0.0, 0.0, 70.0 / 255.0), color);
        vec2 a = vec2(x0, areaTop);
        vec2 d = vec2(endX, areaTop + areaH) - a;
        float dist = abs(d.x * (p.y - a.y) - d.y * (p.x - a.x)) / max(length(d), 1e-6);
        if (dist <= 0.5) color = over(vec4(vec3(150.0 / 255.0), 150.0 / 255.0), color);
      }
    }
  }
  if (color.a <= 0.0) discard;
  o_color = color * v_alpha.y;
}`;

/** One attribute of an instance buffer. */
interface Attribute {
  location: number;
  size: number;
  /** Offset in elements (floats or integers). */
  offset: number;
  integer?: boolean;
}

/** An instanced program and its vertex array. */
class Pass {
  readonly program: WebGLProgram;
  readonly vao: WebGLVertexArrayObject;
  readonly floats: WebGLBuffer;
  readonly ints: WebGLBuffer | null;
  private readonly view: WebGLUniformLocation | null;
  private readonly size: WebGLUniformLocation | null;
  count = 0;
  uploaded = -1;

  constructor(
    private readonly gl: WebGL2RenderingContext,
    vs: string,
    fs: string,
    floatStride: number,
    floatAttributes: Attribute[],
    intStride = 0,
    intAttributes: Attribute[] = [],
  ) {
    this.program = link(gl, vs, fs);
    this.view = gl.getUniformLocation(this.program, "u_view");
    this.size = gl.getUniformLocation(this.program, "u_size");
    const vao = gl.createVertexArray();
    const floats = gl.createBuffer();
    if (!vao || !floats) throw new Error("WebGL2 could not create a buffer");
    this.vao = vao;
    this.floats = floats;
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, floats);
    for (const a of floatAttributes) {
      gl.enableVertexAttribArray(a.location);
      gl.vertexAttribPointer(a.location, a.size, gl.FLOAT, false, floatStride * 4, a.offset * 4);
      gl.vertexAttribDivisor(a.location, 1);
    }
    this.ints = null;
    if (intStride > 0) {
      const ints = gl.createBuffer();
      if (!ints) throw new Error("WebGL2 could not create a buffer");
      this.ints = ints;
      gl.bindBuffer(gl.ARRAY_BUFFER, ints);
      for (const a of intAttributes) {
        gl.enableVertexAttribArray(a.location);
        gl.vertexAttribIPointer(a.location, a.size, gl.UNSIGNED_INT, intStride * 4, a.offset * 4);
        gl.vertexAttribDivisor(a.location, 1);
      }
    }
    gl.bindVertexArray(null);
  }

  upload(floats: Float32Array, count: number, stride: number, ints?: Uint32Array, intStride = 0): void {
    const { gl } = this;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.floats);
    gl.bufferData(gl.ARRAY_BUFFER, floats.subarray(0, count * stride), gl.DYNAMIC_DRAW);
    if (this.ints && ints) {
      gl.bindBuffer(gl.ARRAY_BUFFER, this.ints);
      gl.bufferData(gl.ARRAY_BUFFER, ints.subarray(0, count * intStride), gl.DYNAMIC_DRAW);
    }
    this.count = count;
  }

  draw(view: Float32Array, width: number, height: number): void {
    if (this.count === 0) return;
    const { gl } = this;
    gl.useProgram(this.program);
    gl.uniform4fv(this.view, view);
    gl.uniform2f(this.size, width, height);
    gl.bindVertexArray(this.vao);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, this.count);
    gl.bindVertexArray(null);
  }
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("WebGL2 could not create a shader");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(`shader: ${gl.getShaderInfoLog(shader)}`);
  }
  return shader;
}

function link(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const program = gl.createProgram();
  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, vs));
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(`program: ${gl.getProgramInfoLog(program)}`);
  }
  return program;
}

/** The view as the shaders read it. */
export interface RenderView {
  scrollX: number;
  pixelsPerBeat: number;
  scrollY: number;
  /** Device pixels per CSS pixel. */
  scale: number;
}

/** What one frame draws. */
export interface Frame {
  readonly view: RenderView;
  readonly under: Float32Array;
  readonly underCount: number;
  readonly scene: SceneLayer;
  readonly preview: SceneLayer | null;
  readonly over: Float32Array;
  readonly overCount: number;
  readonly atlas: PeaksAtlas;
}

export class WebGLTimeline {
  readonly renderer: string;
  private readonly gl: WebGL2RenderingContext;
  private readonly rects: Pass;
  private readonly overRects: Pass;
  private readonly scene: { clips: Pass; notes: Pass; waves: Pass };
  private readonly preview: { clips: Pass; notes: Pass; waves: Pass };
  private readonly texture: WebGLTexture;
  private atlasVersion = -1;
  private readonly viewUniform = new Float32Array(4);
  private readonly pixel = new Uint8Array(4);
  private readonly background = unit(parseHex(THEME.window));

  constructor(readonly canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      premultipliedAlpha: true,
      preserveDrawingBuffer: false,
      powerPreference: "high-performance",
    });
    if (!gl) throw new Error("WebGL2 is not available");
    this.gl = gl;
    this.renderer = String(gl.getParameter(gl.RENDERER));
    const rect = (): Pass =>
      new Pass(gl, RECT_VS, RECT_FS, RECT_FLOATS, [
        { location: 0, size: 4, offset: 0 },
        { location: 1, size: 4, offset: 4 },
      ]);
    this.rects = rect();
    this.overRects = rect();
    const passes = () => ({
      clips: new Pass(gl, CLIP_VS, CLIP_FS, CLIP_FLOATS, [
        { location: 0, size: 4, offset: 0 },
        { location: 1, size: 3, offset: 4 },
        { location: 2, size: 3, offset: 7 },
        { location: 3, size: 3, offset: 10 },
        { location: 4, size: 2, offset: 13 },
      ]),
      notes: new Pass(gl, NOTE_VS, NOTE_FS, NOTE_FLOATS, [
        { location: 0, size: 4, offset: 0 },
        { location: 1, size: 4, offset: 4 },
        { location: 2, size: 4, offset: 8 },
        { location: 3, size: 1, offset: 12 },
      ]),
      waves: new Pass(
        gl,
        WAVE_VS,
        WAVE_FS,
        WAVE_FLOATS,
        [
          { location: 0, size: 4, offset: 0 },
          { location: 1, size: 4, offset: 4 },
          { location: 2, size: 4, offset: 8 },
          { location: 3, size: 4, offset: 12 },
          { location: 4, size: 2, offset: 16 },
        ],
        WAVE_INTS,
        [
          { location: 5, size: 4, offset: 0, integer: true },
          { location: 6, size: 1, offset: 4, integer: true },
        ],
      ),
    });
    this.scene = passes();
    this.preview = passes();
    const texture = gl.createTexture();
    if (!texture) throw new Error("WebGL2 could not create a texture");
    this.texture = texture;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RG32F, 1, 1, 0, gl.RG, gl.FLOAT, new Float32Array(2));
    const waves = this.scene.waves.program;
    gl.useProgram(waves);
    gl.uniform1i(gl.getUniformLocation(waves, "u_peaks"), 0);
    gl.useProgram(this.preview.waves.program);
    gl.uniform1i(gl.getUniformLocation(this.preview.waves.program, "u_peaks"), 0);
  }

  private uploadLayer(passes: { clips: Pass; notes: Pass; waves: Pass }, layer: SceneLayer): void {
    if (passes.clips.uploaded !== layer.clipsVersion) {
      passes.clips.upload(layer.clips.floats, layer.clips.count, CLIP_FLOATS);
      passes.clips.uploaded = layer.clipsVersion;
    }
    if (passes.notes.uploaded !== layer.contentVersion) {
      passes.notes.upload(layer.notes.floats, layer.notes.count, NOTE_FLOATS);
      passes.waves.upload(layer.waves.floats, layer.waves.count, WAVE_FLOATS, layer.waves.ints, WAVE_INTS);
      passes.notes.uploaded = layer.contentVersion;
    }
  }

  private uploadAtlas(atlas: PeaksAtlas): void {
    if (this.atlasVersion === atlas.version) return;
    const { gl } = this;
    const texels = Math.max(1, atlas.texels);
    const rows = Math.ceil(texels / ATLAS_WIDTH);
    const max = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
    if (rows > max) throw new Error(`the peaks need ${rows} texture rows, at most ${max}`);
    const data = new Float32Array(rows * ATLAS_WIDTH * 2);
    data.set(atlas.data.subarray(0, Math.min(atlas.data.length, data.length)));
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RG32F, ATLAS_WIDTH, rows, 0, gl.RG, gl.FLOAT, data);
    this.atlasVersion = atlas.version;
  }

  draw(frame: Frame): void {
    const { gl } = this;
    const width = this.canvas.width;
    const height = this.canvas.height;
    this.uploadAtlas(frame.atlas);
    this.uploadLayer(this.scene, frame.scene);
    if (frame.preview) this.uploadLayer(this.preview, frame.preview);
    this.rects.upload(frame.under, frame.underCount, RECT_FLOATS);
    this.overRects.upload(frame.over, frame.overCount, RECT_FLOATS);
    const v = frame.view;
    this.viewUniform.set([v.scrollX, v.pixelsPerBeat, v.scrollY, v.scale]);
    gl.viewport(0, 0, width, height);
    gl.clearColor(this.background[0], this.background[1], this.background[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    this.rects.draw(this.viewUniform, width, height);
    for (const passes of frame.preview ? [this.scene, this.preview] : [this.scene]) {
      passes.clips.draw(this.viewUniform, width, height);
      passes.notes.draw(this.viewUniform, width, height);
      passes.waves.draw(this.viewUniform, width, height);
    }
    this.overRects.draw(this.viewUniform, width, height);
  }

  /** Waits until the last draw has run on the GPU, through a one-pixel read-back. */
  sync(): void {
    this.gl.readPixels(0, 0, 1, 1, this.gl.RGBA, this.gl.UNSIGNED_BYTE, this.pixel);
  }

  /** The canvas's pixels (tests), bottom row first. */
  readPixels(): Uint8Array {
    const { gl } = this;
    const out = new Uint8Array(this.canvas.width * this.canvas.height * 4);
    gl.readPixels(0, 0, this.canvas.width, this.canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, out);
    return out;
  }
}

