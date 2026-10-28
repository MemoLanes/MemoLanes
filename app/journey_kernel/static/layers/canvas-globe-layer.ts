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

/** All globe pixels come from the shared Canvas painter. This layer only places
 * those images on one continuous mesh, including both Mercator/polar seams.
 * CPU projection avoids GPU trigonometry/cancellation errors at native detail. */
export class CanvasGlobeLayer implements CustomLayerInterface {
  readonly type = "custom" as const;
  readonly renderingMode = "2d" as const;
  private program: WebGLProgram | null = null;
  private buffer: WebGLBuffer | null = null;
  private data: PolarCanvasProvider | null = null;
  private mercator: CanvasImage | null = null;
  private readonly charts = [
    createChart("south", -90, -POLAR_SEAM_LATITUDE),
    createChart("mercator", -POLAR_SEAM_LATITUDE, POLAR_SEAM_LATITUDE),
    createChart("north", POLAR_SEAM_LATITUDE, 90),
  ];
  private meshKey = "";
  private attributes: number[] = [];
  private uniforms: Record<string, WebGLUniformLocation | null> = {};
  private readonly map: MaplibreMap;
  readonly id: string;
  private readonly rasterLayerId: string;
  private readonly bg: RGBAColor;
  private readonly fg: RGBAColor;
  private readonly tracks: JourneyTileProvider;
  private globeVisible = false;
  private readonly updateVisibility = () => {
    const visible = (this.map.style?.projection?.transitionState ?? 0) > 0.98;
    if (visible === this.globeVisible) return;
    this.globeVisible = visible;
    if (this.map.getLayer(this.rasterLayerId))
      this.map.setPaintProperty(
        this.rasterLayerId,
        "raster-opacity",
        visible ? 0 : 1,
      );
  };

  constructor(
    map: MaplibreMap,
    id: string,
    rasterLayerId: string,
    bg: RGBAColor,
    fg: RGBAColor,
    tracks: JourneyTileProvider,
  ) {
    this.map = map;
    this.id = id;
    this.rasterLayerId = rasterLayerId;
    this.bg = bg;
    this.fg = fg;
    this.tracks = tracks;
  }

  setMercator(canvas: HTMLCanvasElement, range: TileRange): void {
    this.mercator = { canvas, range };
    this.map.triggerRepaint();
  }

  isReadyForDisplay(): boolean {
    return !this.globeVisible || this.data?.isReadyForDisplay() === true;
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
    this.map.on("render", this.updateVisibility);
    this.updateVisibility();
  }

  render(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    args: CustomRenderMethodInput,
  ): void {
    this.updateVisibility();
    if (args.shaderData.variantName !== "globe" || !this.globeVisible) return;
    if (!this.program) this.setup(gl);
    if (!this.program || !this.buffer) return;
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
      Object.values(this.map.getPadding()),
      canvas.clientWidth,
      canvas.clientHeight,
      this.charts.map((chart) => this.getImage(chart.plane)?.range),
      Array.from(args.defaultProjectionData.mainMatrix),
    ].join("/");
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    if (key !== this.meshKey) {
      gl.bufferData(gl.ARRAY_BUFFER, this.buildMesh(args), gl.DYNAMIC_DRAW);
      this.meshKey = key;
    }
    gl.useProgram(this.program);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    const depth = gl.isEnabled(gl.DEPTH_TEST),
      cull = gl.isEnabled(gl.CULL_FACE);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.CULL_FACE);
    gl.uniform4f(
      this.uniforms.bg,
      this.bg[0] * this.bg[3],
      this.bg[1] * this.bg[3],
      this.bg[2] * this.bg[3],
      this.bg[3],
    );
    gl.uniform1i(this.uniforms.texture, 0);
    gl.activeTexture(gl.TEXTURE0);
    for (let i = 0; i < 3; i++) {
      gl.enableVertexAttribArray(this.attributes[i]);
      gl.vertexAttribPointer(
        this.attributes[i],
        i === 2 ? 1 : 2,
        gl.FLOAT,
        false,
        20,
        i * 8,
      );
    }
    let offset = 0;
    try {
      for (const chart of this.charts) {
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
        gl.uniform1f(this.uniforms.hasImage, image ? 1 : 0);
        gl.uniform1f(
          this.uniforms.wrap,
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

  private buildMesh(args: CustomRenderMethodInput): Float32Array {
    const bounds = this.map.getBounds(),
      canvas = this.map.getCanvas();
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
    if (east - west >= 360 || north === 90 || south === -90) {
      west = -180;
      east = 180;
    }
    const cols = 128,
      rows = 48;
    const vertices: number[] = [];
    for (const chart of this.charts) {
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
          const p = this.map.project([lng, lat]),
            radians = (lat * Math.PI) / 180,
            angle = (lng * Math.PI) / 180;
          let u: number, v: number;
          if (chart.plane === "mercator") {
            u = (lng + 180) / 360;
            v = (1 - Math.asinh(Math.tan(radians)) / Math.PI) / 2;
          } else [u, v] = polarUV(lng, lat);
          grid.push([
            (p.x * 2) / canvas.clientWidth - 1,
            1 - (p.y * 2) / canvas.clientHeight,
            (u * n - target[0]) / target[2],
            (v * n - target[1]) / target[3],
            Math.sin(angle) * Math.cos(radians) * plane[0] +
              Math.sin(radians) * plane[1] +
              Math.cos(angle) * Math.cos(radians) * plane[2] +
              plane[3],
          ]);
        }
      for (let j = 0; j < rows; j++)
        for (let i = 0; i < cols; i++) {
          const a = j * (cols + 1) + i,
            b = a + cols + 1;
          for (const index of [a, b, a + 1, a + 1, b, b + 1])
            vertices.push(...grid[index]);
        }
      chart.vertexCount = (vertices.length - start) / 5;
    }
    return new Float32Array(vertices);
  }

  private setup(gl: WebGLRenderingContext | WebGL2RenderingContext): void {
    const program = gl.createProgram();
    if (!program) return;
    const sources = [
      [
        gl.VERTEX_SHADER,
        `#version 300 es
      in vec2 a_position; in vec2 a_uv; in float a_horizon;
      out highp vec2 v_uv; out highp float v_horizon;
      void main(){gl_Position=vec4(a_position,0.0,1.0);v_uv=a_uv;v_horizon=a_horizon;}`,
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
        return;
      }
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      gl.attachShader(program, shader);
      gl.deleteShader(shader);
    }
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error("Canvas globe shader:", gl.getProgramInfoLog(program));
      gl.deleteProgram(program);
      return;
    }
    this.program = program;
    this.buffer = gl.createBuffer();
    this.attributes = ["a_position", "a_uv", "a_horizon"].map((name) =>
      gl.getAttribLocation(program, name),
    );
    for (const name of ["bg", "texture", "hasImage", "wrap"])
      this.uniforms[name] = gl.getUniformLocation(program, `u_${name}`);
    for (const chart of this.charts) {
      const texture = gl.createTexture();
      if (!texture) throw new Error("Unable to allocate Canvas globe texture");
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
    this.map.off("render", this.updateVisibility);
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
    gl.deleteProgram(this.program);
    this.buffer = null;
    this.program = null;
    this.mercator = null;
  }
}
