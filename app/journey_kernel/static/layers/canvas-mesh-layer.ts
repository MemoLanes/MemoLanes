import type {
  CustomLayerInterface,
  CustomRenderMethodInput,
  Map as MaplibreMap,
} from "maplibre-gl";
import type { JourneyTileProvider } from "../journey-tile-provider";
import type { RGBAColor } from "./journey-layer-interface";
import type { TileRange } from "./tile-range";
import {
  polarUV,
  POLAR_SEAM_LATITUDE,
  type PolarHemisphere,
} from "../polar-projection";
import { PolarCanvasProvider } from "../polar-canvas-provider";

interface CanvasImage {
  canvas: HTMLCanvasElement;
  range: TileRange;
}
type CanvasPlane = "mercator" | PolarHemisphere;
interface ChartSurface {
  plane: CanvasPlane;
  latitude: readonly [number, number];
  texture: WebGLTexture | null;
  uploaded: CanvasImage | null;
  vertexCount: number;
}

interface MeshProgram {
  program: WebGLProgram;
  attributes: number[];
  uniforms: Record<string, WebGLUniformLocation | null>;
}

const VERTEX_FLOATS = 8; // clip x/y/w, texture u/v, horizon, Mercator x/y

/** Keep projection arithmetic in float64; only the final clip positions go to GL. */
function projectPosition(
  matrix: ArrayLike<number>,
  x: number,
  y: number,
  z: number,
): [number, number, number] {
  return [
    matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12],
    matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13],
    matrix[3] * x + matrix[7] * y + matrix[11] * z + matrix[15],
  ];
}

function createChart(
  plane: CanvasPlane,
  south: number,
  north: number,
): ChartSurface {
  return {
    plane,
    latitude: [south, north],
    texture: null,
    uploaded: null,
    vertexCount: 0,
  };
}

/** One renderer places Canvas images in both flat and spherical projections.
 * All charts share a mesh at the polar seams; no other layer covers these pixels.
 * CPU projection avoids GPU trigonometry/cancellation errors at native detail. */
export class CanvasMeshLayer implements CustomLayerInterface {
  readonly type = "custom" as const;
  readonly renderingMode = "2d" as const;
  private readonly programs = new Map<string, MeshProgram>();
  private buffer: WebGLBuffer | null = null;
  private data: PolarCanvasProvider | null = null;
  private mercator: CanvasImage | null = null;
  private readonly charts = [
    createChart("south", -90, -POLAR_SEAM_LATITUDE),
    createChart("mercator", -POLAR_SEAM_LATITUDE, POLAR_SEAM_LATITUDE),
    createChart("north", POLAR_SEAM_LATITUDE, 90),
  ];
  private meshKey = "";
  private readonly map: MaplibreMap;
  readonly id: string;
  private readonly bg: RGBAColor;
  private readonly fg: RGBAColor;
  private readonly tracks: JourneyTileProvider;

  constructor(
    map: MaplibreMap,
    id: string,
    bg: RGBAColor,
    fg: RGBAColor,
    tracks: JourneyTileProvider,
  ) {
    this.map = map;
    this.id = id;
    this.bg = bg;
    this.fg = fg;
    this.tracks = tracks;
  }

  setMercator(canvas: HTMLCanvasElement, range: TileRange): void {
    this.mercator = { canvas, range };
    this.map.triggerRepaint();
  }

  isReadyForDisplay(): boolean {
    return (
      !this.map.style?.projection?.transitionState ||
      this.data?.isReadyForDisplay() === true
    );
  }

  private getImage(plane: CanvasPlane): CanvasImage | null {
    return plane === "mercator"
      ? this.mercator
      : (this.data?.get(plane) ?? null);
  }

  onAdd(): void {
    this.data = new PolarCanvasProvider(
      this.map,
      this.tracks,
      this.bg,
      this.fg,
    );
  }

  render(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    args: CustomRenderMethodInput,
  ): void {
    // Select coverage from this frame's projection, with no queued visibility
    // state. The flat world ends at the Mercator boundary; the sphere has caps.
    const charts = this.charts.filter(
      (chart) =>
        chart.plane === "mercator" || args.shaderData.variantName === "globe",
    );
    const variant = args.shaderData.variantName;
    const bindings = this.programs.get(variant) ?? this.createProgram(gl, args);
    if (!bindings) return;
    if (!this.buffer) this.setupResources(gl);
    if (!this.buffer) return;
    const { program, attributes, uniforms } = bindings;
    const bounds = this.map.getBounds(),
      center = this.map.getCenter(),
      canvas = this.map.getCanvas();
    const key = [
      bounds.toArray(),
      center.lng,
      center.lat,
      this.map.getZoom(),
      this.map.getBearing(),
      this.map.getPitch(),
      this.map.getRoll(),
      this.map.getRenderWorldCopies(),
      Object.values(this.map.getPadding()),
      canvas.clientWidth,
      canvas.clientHeight,
      charts.map((chart) => [chart.plane, this.getImage(chart.plane)?.range]),
      args.defaultProjectionData.projectionTransition,
      Array.from(args.modelViewProjectionMatrix),
      Array.from(args.defaultProjectionData.mainMatrix),
      Array.from(args.defaultProjectionData.fallbackMatrix),
    ].join("/");
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    if (key !== this.meshKey) {
      gl.bufferData(
        gl.ARRAY_BUFFER,
        this.buildMesh(args, charts),
        gl.DYNAMIC_DRAW,
      );
      this.meshKey = key;
    }
    gl.useProgram(program);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    const depth = gl.isEnabled(gl.DEPTH_TEST),
      cull = gl.isEnabled(gl.CULL_FACE);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.CULL_FACE);
    gl.uniform4f(
      uniforms.bg,
      this.bg[0] * this.bg[3],
      this.bg[1] * this.bg[3],
      this.bg[2] * this.bg[3],
      this.bg[3],
    );
    // MapLibre's shader owns the intermediate projection. A CPU reimplementation
    // can drift from the basemap because GPU arithmetic differs. At the endpoints
    // use float64 projection to keep native-detail tracks stable, especially at poles.
    const projection = args.defaultProjectionData;
    gl.uniform1f(
      uniforms.useMapProjection,
      variant === "globe" && projection.projectionTransition < 1 ? 1 : 0,
    );
    gl.uniformMatrix4fv(
      uniforms.projection_matrix,
      false,
      projection.mainMatrix,
    );
    gl.uniformMatrix4fv(
      uniforms.projection_fallback_matrix,
      false,
      projection.fallbackMatrix,
    );
    gl.uniform4fv(
      uniforms.projection_tile_mercator_coords,
      projection.tileMercatorCoords,
    );
    gl.uniform4fv(uniforms.projection_clipping_plane, projection.clippingPlane);
    gl.uniform1f(
      uniforms.projection_transition,
      projection.projectionTransition,
    );
    gl.uniform1i(uniforms.texture, 0);
    gl.activeTexture(gl.TEXTURE0);
    const layout = [
      [3, 0],
      [2, 3],
      [1, 5],
      [2, 6],
    ] as const;
    for (let i = 0; i < layout.length; i++) {
      const [size, offset] = layout[i];
      gl.enableVertexAttribArray(attributes[i]);
      gl.vertexAttribPointer(
        attributes[i],
        size,
        gl.FLOAT,
        false,
        VERTEX_FLOATS * Float32Array.BYTES_PER_ELEMENT,
        offset * Float32Array.BYTES_PER_ELEMENT,
      );
    }
    let offset = 0;
    try {
      for (const chart of charts) {
        const image = this.getImage(chart.plane);
        gl.bindTexture(gl.TEXTURE_2D, chart.texture);
        if (image && image !== chart.uploaded) {
          const premultiplied = gl.getParameter(
              gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL,
            ),
            flip = gl.getParameter(gl.UNPACK_FLIP_Y_WEBGL);
          gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
          gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
          gl.texImage2D(
            gl.TEXTURE_2D,
            0,
            gl.RGBA,
            gl.RGBA,
            gl.UNSIGNED_BYTE,
            image.canvas,
          );
          gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, premultiplied);
          gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, flip);
          chart.uploaded = image;
        }
        gl.uniform1f(uniforms.hasImage, image ? 1 : 0);
        gl.uniform1f(
          uniforms.wrap,
          chart.plane === "mercator" && image
            ? 2 ** image.range[4] / image.range[2]
            : 0,
        );
        gl.drawArrays(gl.TRIANGLES, offset, chart.vertexCount);
        offset += chart.vertexCount;
      }
    } finally {
      if (depth) gl.enable(gl.DEPTH_TEST);
      if (cull) gl.enable(gl.CULL_FACE);
      gl.bindTexture(gl.TEXTURE_2D, null);
    }
  }

  private buildMesh(
    args: CustomRenderMethodInput,
    charts: readonly ChartSurface[],
  ): Float32Array {
    const bounds = this.map.getBounds();
    const plane = args.defaultProjectionData.clippingPlane;
    // Expand beyond the viewport so curved edges and tilted views stay covered.
    const latPad = Math.max(
      0.00001,
      (bounds.getNorth() - bounds.getSouth()) * 0.25,
    );
    const south = Math.max(-90, bounds.getSouth() - latPad),
      north = Math.min(90, bounds.getNorth() + latPad);
    let west = bounds.getWest(),
      east = bounds.getEast();
    if (east < west) east += 360;
    const lngPad = (east - west) * 0.25;
    west -= lngPad;
    east += lngPad;
    const spherical = args.shaderData.variantName === "globe";
    if (spherical && (east - west >= 360 || north === 90 || south === -90)) {
      west = -180;
      east = 180;
    } else if (!spherical && !this.map.getRenderWorldCopies()) {
      west = Math.max(-180, west);
      east = Math.min(180, east);
    }
    // Mercator is planar: a quad with clip-space w preserves perspective even
    // when pitched. Only the curved sphere needs subdivision.
    const cols = spherical ? 128 : 1,
      rows = spherical ? 48 : 1;
    const vertices: number[] = [];
    for (const chart of charts) {
      const start = vertices.length;
      const low = Math.max(south, chart.latitude[0]),
        high = Math.min(north, chart.latitude[1]);
      chart.vertexCount = 0;
      if (high <= low) continue;
      const target = this.getImage(chart.plane)?.range ?? [0, 0, 1, 1, 0],
        n = 2 ** target[4];
      const grid: number[][] = [];
      for (let j = 0; j <= rows; j++)
        for (let i = 0; i <= cols; i++) {
          const lng = west + ((east - west) * i) / cols,
            lat = low + ((high - low) * j) / rows;
          const radians = (lat * Math.PI) / 180,
            angle = (lng * Math.PI) / 180;
          const sphere: [number, number, number] = [
            Math.sin(angle) * Math.cos(radians),
            Math.sin(radians),
            Math.cos(angle) * Math.cos(radians),
          ];
          const mercatorU = (lng + 180) / 360,
            mercatorV = (1 - Math.asinh(Math.tan(radians)) / Math.PI) / 2;
          const projection = args.defaultProjectionData;
          const position = spherical
            ? projectPosition(args.modelViewProjectionMatrix, ...sphere)
            : projectPosition(projection.mainMatrix, mercatorU, mercatorV, 0);
          const [u, v] =
            chart.plane === "mercator"
              ? [mercatorU, mercatorV]
              : polarUV(lng, lat);
          grid.push([
            ...position,
            (u * n - target[0]) / target[2],
            (v * n - target[1]) / target[3],
            sphere[0] * plane[0] +
              sphere[1] * plane[1] +
              sphere[2] * plane[2] +
              plane[3],
            mercatorU,
            mercatorV,
          ]);
        }
      for (let j = 0; j < rows; j++)
        for (let i = 0; i < cols; i++) {
          const a = j * (cols + 1) + i,
            b = a + cols + 1;
          for (const index of [a, b, a + 1, a + 1, b, b + 1])
            vertices.push(...grid[index]);
        }
      chart.vertexCount = (vertices.length - start) / VERTEX_FLOATS;
    }
    return new Float32Array(vertices);
  }

  private createProgram(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    args: CustomRenderMethodInput,
  ): MeshProgram | null {
    const program = gl.createProgram();
    if (!program) return null;
    const sources = [
      [
        gl.VERTEX_SHADER,
        `#version 300 es
      ${args.shaderData.define}
      ${args.shaderData.vertexShaderPrelude}
      in vec3 a_position; in vec2 a_uv; in float a_horizon; in vec2 a_mercator;
      uniform float u_useMapProjection;
      out highp vec2 v_uv; out highp float v_horizon;
      void main(){
        gl_Position=u_useMapProjection>0.5 ? projectTile(a_mercator) : vec4(a_position.xy,0.0,a_position.z);
        v_uv=a_uv;v_horizon=a_horizon;
      }`,
      ],
      [
        gl.FRAGMENT_SHADER,
        `#version 300 es
      precision highp float;
      in vec2 v_uv; in float v_horizon;
      uniform sampler2D u_texture; uniform vec4 u_bg; uniform float u_hasImage; uniform float u_wrap;
      out vec4 color;
      void main(){if(v_horizon<0.0)discard;color=u_bg;
      vec2 uv=v_uv;
      if(u_wrap>0.0) uv.x+=floor((0.5-uv.x)/u_wrap+0.5)*u_wrap;
      if(u_hasImage>0.5 && all(greaterThanEqual(uv,vec2(0.0))) && all(lessThanEqual(uv,vec2(1.0))))color=texture(u_texture,uv);}`,
      ],
    ] as const;
    for (const [type, source] of sources) {
      const shader = gl.createShader(type);
      if (!shader) {
        gl.deleteProgram(program);
        return null;
      }
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      gl.attachShader(program, shader);
      gl.deleteShader(shader);
    }
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error("Canvas mesh shader:", gl.getProgramInfoLog(program));
      gl.deleteProgram(program);
      return null;
    }
    const attributes = ["a_position", "a_uv", "a_horizon", "a_mercator"].map(
      (name) => gl.getAttribLocation(program, name),
    );
    const uniforms: MeshProgram["uniforms"] = {};
    for (const name of [
      "bg",
      "texture",
      "hasImage",
      "wrap",
      "useMapProjection",
      "projection_matrix",
      "projection_fallback_matrix",
      "projection_tile_mercator_coords",
      "projection_clipping_plane",
      "projection_transition",
    ])
      uniforms[name] = gl.getUniformLocation(program, `u_${name}`);
    const bindings = { program, attributes, uniforms };
    this.programs.set(args.shaderData.variantName, bindings);
    return bindings;
  }

  private setupResources(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
  ): void {
    this.buffer = gl.createBuffer();
    for (const chart of this.charts) {
      const texture = gl.createTexture();
      if (!texture) throw new Error("Unable to allocate Canvas mesh texture");
      gl.bindTexture(gl.TEXTURE_2D, texture);
      for (const p of [gl.TEXTURE_WRAP_S, gl.TEXTURE_WRAP_T])
        gl.texParameteri(gl.TEXTURE_2D, p, gl.CLAMP_TO_EDGE);
      for (const p of [gl.TEXTURE_MIN_FILTER, gl.TEXTURE_MAG_FILTER])
        gl.texParameteri(gl.TEXTURE_2D, p, gl.LINEAR);
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RGBA,
        1,
        1,
        0,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        new Uint8Array(4),
      );
      chart.texture = texture;
    }
  }

  onRemove(
    _map: MaplibreMap,
    gl: WebGLRenderingContext | WebGL2RenderingContext,
  ): void {
    this.data?.dispose();
    this.data = null;
    for (const chart of this.charts) {
      gl.deleteTexture(chart.texture);
      chart.texture = null;
      chart.uploaded = null;
      chart.vertexCount = 0;
    }
    this.meshKey = "";
    gl.deleteBuffer(this.buffer);
    for (const { program } of this.programs.values()) gl.deleteProgram(program);
    this.programs.clear();
    this.buffer = null;
    this.mercator = null;
  }
}
