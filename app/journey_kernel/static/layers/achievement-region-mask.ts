import { GeoJSONVT } from "@maplibre/geojson-vt";
import {
  createTileMesh,
  type CustomLayerInterface,
  type CustomRenderMethodInput,
  type Map as MapLibreMap,
  type UnwrappedTileIDLiteral,
} from "maplibre-gl";
import type { AchievementRegionFeature } from "../achievement-region-overlay";
import { JOURNEY_LAYER_ID } from "./journey-layer-interface";
import { tileXYZToKey } from "./utils";
import { createLinearClampedTexture2D } from "./webgl-texture";

const extent = 8192;
const tileSize = 512;
const maxZoom = 24;
const tileCacheSize = 64;
const maximumMaskPixels = 2_000_000;

const masks = new WeakMap<MapLibreMap, AchievementRegionMask>();

export function achievementFogMask(
  map: MapLibreMap,
  gl: WebGLRenderingContext,
) {
  return masks.get(map)?.textureFor(gl) ?? null;
}

function createProgram(
  gl: WebGL2RenderingContext,
  vertex: string,
  fragment: string,
) {
  const program = gl.createProgram();
  try {
    for (const [type, source] of [
      [gl.VERTEX_SHADER, vertex],
      [gl.FRAGMENT_SHADER, fragment],
    ] as const) {
      const shader = gl.createShader(type)!;
      try {
        gl.shaderSource(shader, source);
        gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
          throw new Error(
            gl.getShaderInfoLog(shader) ??
              "Cannot compile achievement mask shader",
          );
        }
        gl.attachShader(program, shader);
      } finally {
        gl.deleteShader(shader);
      }
    }
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      throw new Error(
        gl.getProgramInfoLog(program) ?? "Cannot link achievement mask shader",
      );
    }
    return program;
  } catch (error) {
    gl.deleteProgram(program);
    throw error;
  }
}

const maskFragment = `#version 300 es
precision mediump float;
in vec2 v_uv;
uniform sampler2D u_tile;
out vec4 fragColor;
void main() {
  fragColor = vec4(texture(u_tile, v_uv).a, 0.0, 0.0, 1.0);
}`;

const screenVertex = `#version 300 es
out vec2 v_uv;
void main() {
  v_uv = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
  gl_Position = vec4(2.0 * v_uv - 1.0, 0.0, 1.0);
}`;

// The coast is not the edge of the glow. Seed light well inside the shape,
// then diffuse it in screen space without blurring the actual map.
const seedFragment = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_source;
uniform vec2 u_cssPixel;
out vec4 color;
float coverage(vec2 offset) {
  return texture(u_source, v_uv + offset * u_cssPixel).r;
}
float ring(float r) {
  float d = r * 0.70710678;
  return (coverage(vec2(r,0)) + coverage(vec2(-r,0)) +
    coverage(vec2(0,r)) + coverage(vec2(0,-r)) +
    coverage(vec2(d,d)) + coverage(vec2(-d,-d)) +
    coverage(vec2(d,-d)) + coverage(vec2(-d,d))) / 8.0;
}
void main() {
  float center = coverage(vec2(0));
  float interior = center > 0.0 ? smoothstep(0.67, 0.98, 0.5 * (ring(8.0) + ring(16.0))) : 0.0;
  color = vec4(center * mix(0.22, 1.0, interior), 0.0, 0.0, 1.0);
}`;

const blurFragment = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_source;
uniform vec2 u_step;
out vec4 color;
void main() {
  float value = 0.0;
  float total = 0.0;
  for (int i = -6; i <= 6; i++) {
    float x = float(i);
    const float sigma = 8.0 / 3.0;
    float weight = exp(-0.5 * x*x / (sigma*sigma));
    value += weight * texture(u_source, v_uv + x * u_step).r;
    total += weight;
  }
  color = vec4(value / total, 0.0, 0.0, 1.0);
}`;

type MaskTarget = {
  texture: WebGLTexture;
  framebuffer: WebGLFramebuffer;
  width: number;
  height: number;
};

function createMaskTarget(
  gl: WebGL2RenderingContext,
  width: number,
  height: number,
): MaskTarget | undefined {
  const mask = createLinearClampedTexture2D(gl);
  if (!mask) return;
  gl.texImage2D(
    gl.TEXTURE_2D,
    0,
    gl.R8,
    width,
    height,
    0,
    gl.RED,
    gl.UNSIGNED_BYTE,
    null,
  );
  const framebuffer = gl.createFramebuffer();
  if (!framebuffer) {
    gl.deleteTexture(mask);
    return;
  }
  gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
  gl.framebufferTexture2D(
    gl.FRAMEBUFFER,
    gl.COLOR_ATTACHMENT0,
    gl.TEXTURE_2D,
    mask,
    0,
  );
  return { texture: mask, framebuffer, width, height };
}

function deleteMaskTarget(
  gl: WebGL2RenderingContext,
  target?: MaskTarget,
): void {
  if (!target) return;
  gl.deleteFramebuffer(target.framebuffer);
  gl.deleteTexture(target.texture);
}

function screenProgram(
  gl: WebGL2RenderingContext,
  fragment: string,
  offset: string,
) {
  const program = createProgram(gl, screenVertex, fragment);
  return {
    program,
    source: gl.getUniformLocation(program, "u_source"),
    offset: gl.getUniformLocation(program, offset),
  };
}

type ProjectionProgram = ReturnType<typeof projectionProgram>;
function projectionProgram(
  gl: WebGL2RenderingContext,
  args: CustomRenderMethodInput,
) {
  const program = createProgram(
    gl,
    `#version 300 es
${args.shaderData.vertexShaderPrelude}
${args.shaderData.define}
layout(location = 0) in vec2 a_position;
out vec2 v_uv;
void main() {
  gl_Position = projectTile(a_position, a_position);
  v_uv = clamp(a_position / ${extent}.0, 0.0, 1.0);
}`,
    maskFragment,
  );
  const uniform = (name: string) => gl.getUniformLocation(program, name);
  return {
    program,
    tile: uniform("u_tile"),
    matrix: uniform("u_projection_matrix"),
    fallback: uniform("u_projection_fallback_matrix"),
    transition: uniform("u_projection_transition"),
    clippingPlane: uniform("u_projection_clipping_plane"),
    tileMercatorCoords: uniform("u_projection_tile_mercator_coords"),
  };
}

function createMesh(
  gl: WebGL2RenderingContext,
  granularity: number,
  north: boolean,
  south: boolean,
) {
  const { vertices, indices } = createTileMesh(
    {
      granularity,
      extendToNorthPole: north,
      extendToSouthPole: south,
    },
    "16bit",
  );
  const vao = gl.createVertexArray();
  const vertexBuffer = gl.createBuffer();
  const indexBuffer = gl.createBuffer();
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.SHORT, false, 0, 0);
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
  return { vao, vertexBuffer, indexBuffer, count: indices.byteLength / 2 };
}

export class AchievementRegionMask implements CustomLayerInterface {
  readonly type = "custom" as const;
  readonly id: string;
  private map?: MapLibreMap;
  private gl?: WebGL2RenderingContext;
  private readonly index: GeoJSONVT;
  private readonly tileTextures = new Map<string, WebGLTexture | null>();
  private readonly programs = new Map<string, ProjectionProgram>();
  private readonly meshes = new Map<number, ReturnType<typeof createMesh>>();
  private tileCanvas?: CanvasRenderingContext2D;
  private target?: MaskTarget;
  private glowTarget?: MaskTarget;
  private glowScratch?: MaskTarget;
  private seed?: ReturnType<typeof screenProgram>;
  private blur?: ReturnType<typeof screenProgram>;
  private hasCoverage = false;

  constructor(id: string, feature: AchievementRegionFeature) {
    this.id = id;
    this.index = new GeoJSONVT(feature, {
      extent,
      maxZoom,
      indexMaxZoom: 0,
      tolerance: 1,
      buffer: 0,
      wrap: false,
    });
  }

  onAdd(map: MapLibreMap): void {
    this.map = map;
    masks.set(map, this);
    map.on("webglcontextlost", this.release);
  }

  onRemove(): void {
    if (this.map && masks.get(this.map) === this) masks.delete(this.map);
    this.map?.off("webglcontextlost", this.release);
    this.release();
    this.map = undefined;
  }

  textureFor(gl: WebGLRenderingContext) {
    return this.hasCoverage && this.gl === gl ? this.glowTarget?.texture : null;
  }

  private release = (): void => {
    const gl = this.gl;
    if (gl && !gl.isContextLost()) {
      for (const tile of this.tileTextures.values()) gl.deleteTexture(tile);
      for (const shader of this.programs.values())
        gl.deleteProgram(shader.program);
      for (const { vao, vertexBuffer, indexBuffer } of this.meshes.values()) {
        gl.deleteVertexArray(vao);
        gl.deleteBuffer(vertexBuffer);
        gl.deleteBuffer(indexBuffer);
      }
      deleteMaskTarget(gl, this.target);
      deleteMaskTarget(gl, this.glowTarget);
      deleteMaskTarget(gl, this.glowScratch);
      if (this.seed) gl.deleteProgram(this.seed.program);
      if (this.blur) gl.deleteProgram(this.blur.program);
    }
    this.tileTextures.clear();
    this.programs.clear();
    this.meshes.clear();
    this.target = this.glowTarget = this.glowScratch = this.gl = undefined;
    this.seed = this.blur = undefined;
    this.hasCoverage = false;
  };

  private tileTexture(
    gl: WebGL2RenderingContext,
    { x, y, z }: UnwrappedTileIDLiteral["canonical"],
  ) {
    const key = tileXYZToKey(x, y, z);
    if (this.tileTextures.has(key)) {
      const cached = this.tileTextures.get(key)!;
      this.tileTextures.delete(key);
      this.tileTextures.set(key, cached);
      return cached;
    }
    const tile = this.index.getTile(z, x, y);
    let result: WebGLTexture | null = null;
    if (tile?.features.length) {
      if (!this.tileCanvas) {
        const canvas = document.createElement("canvas");
        canvas.width = canvas.height = tileSize;
        this.tileCanvas = canvas.getContext("2d")!;
      }
      const ctx = this.tileCanvas;
      ctx.clearRect(0, 0, tileSize, tileSize);
      const scale = tileSize / extent;
      for (const feature of tile.features) {
        if (feature.type !== 3) continue;
        ctx.beginPath();
        for (const ring of feature.geometry) {
          for (let i = 0; i < ring.length; i++) {
            const [x, y] = ring[i];
            if (i === 0) ctx.moveTo(x * scale, y * scale);
            else ctx.lineTo(x * scale, y * scale);
          }
          ctx.closePath();
        }
        ctx.fill("evenodd");
      }
      result = createLinearClampedTexture2D(gl);
      if (!result) return null;
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RGBA,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        ctx.canvas,
      );
    }
    this.tileTextures.set(key, result);
    if (this.tileTextures.size > tileCacheSize) {
      const oldest = this.tileTextures.keys().next().value!;
      gl.deleteTexture(this.tileTextures.get(oldest) ?? null);
      this.tileTextures.delete(oldest);
    }
    return result;
  }

  private bindTarget(gl: WebGL2RenderingContext) {
    const { drawingBufferWidth: w, drawingBufferHeight: h } = gl;
    const scale = Math.min(1, Math.sqrt(maximumMaskPixels / (w * h)));
    const [width, height] = [
      Math.max(1, Math.round(w * scale)),
      Math.max(1, Math.round(h * scale)),
    ];
    if (this.target?.width === width && this.target.height === height) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.target.framebuffer);
      return this.target;
    }
    deleteMaskTarget(gl, this.target);
    return (this.target = createMaskTarget(gl, width, height));
  }

  private drawGlow(
    gl: WebGL2RenderingContext,
    raw: MaskTarget,
    canvas: HTMLCanvasElement,
  ): void {
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (!w || !h) return;
    const width = Math.min(raw.width, w);
    const height = Math.min(raw.height, h);
    if (
      this.glowTarget?.width !== width ||
      this.glowTarget.height !== height ||
      !this.glowScratch
    ) {
      deleteMaskTarget(gl, this.glowTarget);
      deleteMaskTarget(gl, this.glowScratch);
      this.glowTarget = createMaskTarget(gl, width, height);
      this.glowScratch = this.glowTarget
        ? createMaskTarget(gl, width, height)
        : undefined;
    }
    if (!this.glowTarget || !this.glowScratch) {
      this.hasCoverage = false;
      return;
    }
    const seed = (this.seed ??= screenProgram(gl, seedFragment, "u_cssPixel"));
    const blur = (this.blur ??= screenProgram(gl, blurFragment, "u_step"));
    gl.bindVertexArray(null);
    gl.viewport(0, 0, width, height);
    const pass = (
      from: MaskTarget,
      to: MaskTarget,
      shader: ReturnType<typeof screenProgram>,
      dx: number,
      dy: number,
    ) => {
      gl.bindFramebuffer(gl.FRAMEBUFFER, to.framebuffer);
      gl.bindTexture(gl.TEXTURE_2D, from.texture);
      gl.useProgram(shader.program);
      gl.uniform1i(shader.source, 0);
      gl.uniform2f(shader.offset, dx, dy);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };
    pass(raw, this.glowTarget, seed, 1 / w, 1 / h);
    pass(this.glowTarget, this.glowScratch, blur, 3 / w, 0);
    pass(this.glowScratch, this.glowTarget, blur, 0, 3 / h);
  }

  prerender(gl: WebGL2RenderingContext, args: CustomRenderMethodInput): void {
    this.hasCoverage = false;
    if (!this.map || !this.map.getLayer(JOURNEY_LAYER_ID) || gl.isContextLost())
      return;
    if (this.gl !== gl) {
      this.release();
      this.gl = gl;
    }
    const viewport = gl.getParameter(gl.VIEWPORT) as Int32Array;
    const framebuffer = gl.getParameter(
      gl.FRAMEBUFFER_BINDING,
    ) as WebGLFramebuffer | null;
    const flip = gl.getParameter(gl.UNPACK_FLIP_Y_WEBGL) as boolean;
    try {
      gl.activeTexture(gl.TEXTURE0);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      const raw = this.bindTarget(gl);
      if (!raw) return;
      const { width, height } = raw;
      gl.viewport(0, 0, width, height);
      gl.disable(gl.DEPTH_TEST);
      gl.disable(gl.STENCIL_TEST);
      gl.disable(gl.SCISSOR_TEST);
      gl.disable(gl.CULL_FACE);
      gl.disable(gl.BLEND);
      gl.colorMask(true, true, true, true);
      gl.clearBufferfv(gl.COLOR, 0, zero);
      const variant = args.shaderData.variantName;
      const shader = this.programs.get(variant) ?? projectionProgram(gl, args);
      this.programs.set(variant, shader);
      gl.useProgram(shader.program);
      gl.uniform1i(shader.tile, 0);
      const globe = variant === "globe";
      for (const tile of this.map.coveringTiles({
        tileSize,
        maxzoom: maxZoom,
      })) {
        const { canonical } = tile;
        const source = this.tileTexture(gl, canonical);
        if (!source) continue;
        const projection = args.getProjectionData({
          tileID: tile,
          applyGlobeMatrix: true,
        });
        gl.uniformMatrix4fv(shader.matrix, false, projection.mainMatrix);
        gl.uniformMatrix4fv(shader.fallback, false, projection.fallbackMatrix);
        gl.uniform4fv(shader.clippingPlane, projection.clippingPlane);
        gl.uniform4fv(shader.tileMercatorCoords, projection.tileMercatorCoords);
        gl.uniform1f(shader.transition, projection.projectionTransition);
        const granularity = globe ? Math.max(1, 128 >> canonical.z) : 1;
        const count = 2 ** canonical.z;
        const north = globe && canonical.y === 0;
        const south = globe && canonical.y === count - 1;
        const key = granularity * 4 + Number(north) + 2 * Number(south);
        let mesh = this.meshes.get(key);
        if (!mesh) {
          mesh = createMesh(gl, granularity, north, south);
          this.meshes.set(key, mesh);
        }
        gl.bindTexture(gl.TEXTURE_2D, source);
        gl.bindVertexArray(mesh.vao);
        gl.drawElements(gl.TRIANGLES, mesh.count, gl.UNSIGNED_SHORT, 0);
        this.hasCoverage = true;
      }
      if (this.hasCoverage) this.drawGlow(gl, raw, this.map.getCanvas());
    } finally {
      gl.bindVertexArray(null);
      gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
      gl.viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, flip);
    }
  }

  render(): void {
    // Only the Journey shader reads the mask; the basemap is never repainted.
  }
}

const zero = new Float32Array(4);
