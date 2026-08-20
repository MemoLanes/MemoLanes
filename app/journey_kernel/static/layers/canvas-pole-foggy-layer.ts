import type {
  CustomLayerInterface,
  CustomRenderMethodInput,
  Map as MaplibreMap,
} from "maplibre-gl";
import type { RGBAColor } from "./journey-layer-interface";
import { getViewportTileRange } from "./utils";

type ProjectionUniforms = {
  clippingPlane: WebGLUniformLocation | null;
  transition: WebGLUniformLocation | null;
  tileMercatorCoords: WebGLUniformLocation | null;
  matrix: WebGLUniformLocation | null;
  fallbackMatrix: WebGLUniformLocation | null;
};

const NORTH_POLE_Y = -32768;
const SOUTH_POLE_Y = 32767;

/** Minimal pole-only renderer used by Canvas mode. */
export class CanvasPoleFoggyLayer implements CustomLayerInterface {
  readonly id: string;
  readonly type = "custom" as const;
  readonly renderingMode = "2d" as const;

  private readonly map: MaplibreMap;
  private readonly bgColor: Float32Array;
  private program: WebGLProgram | null = null;
  private buffer: WebGLBuffer | null = null;
  private aPos?: number;
  private uBgColor: WebGLUniformLocation | null = null;
  private projectionUniforms?: ProjectionUniforms;
  private meshKey = "";
  private poleMesh = new Float32Array();
  private uploadedPoleMesh: Float32Array | null = null;

  constructor(map: MaplibreMap, id: string, bgColor: RGBAColor) {
    this.map = map;
    this.id = id;
    const [r, g, b, a] = bgColor;
    this.bgColor = new Float32Array([r * a, g * a, b * a, a]);
  }

  onAdd(): void {}

  onRemove(
    _map: MaplibreMap,
    gl: WebGLRenderingContext | WebGL2RenderingContext,
  ): void {
    if (this.program) gl.deleteProgram(this.program);
    if (this.buffer) gl.deleteBuffer(this.buffer);
    this.program = null;
    this.buffer = null;
    this.uBgColor = null;
    this.projectionUniforms = undefined;
    this.uploadedPoleMesh = null;
    this.meshKey = "";
    this.poleMesh = new Float32Array();
  }

  render(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    args: CustomRenderMethodInput,
  ): void {
    if (args.shaderData.variantName !== "globe") return;

    if (!this.program) this.setupShader(gl, args);
    if (
      !this.program ||
      !this.buffer ||
      this.aPos === undefined ||
      !this.projectionUniforms ||
      !this.uBgColor
    ) {
      return;
    }

    const projection = args.defaultProjectionData;
    const uniforms = this.projectionUniforms;

    gl.useProgram(this.program);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.uniform4f(uniforms.clippingPlane, ...projection.clippingPlane);
    gl.uniform1f(uniforms.transition, projection.projectionTransition);
    gl.uniform4f(uniforms.tileMercatorCoords, ...projection.tileMercatorCoords);
    gl.uniformMatrix4fv(uniforms.matrix, false, projection.mainMatrix);
    gl.uniformMatrix4fv(
      uniforms.fallbackMatrix,
      false,
      projection.fallbackMatrix,
    );
    gl.uniform4fv(this.uBgColor, this.bgColor);

    const poleMesh = this.getPoleMesh();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    if (this.uploadedPoleMesh !== poleMesh) {
      gl.bufferData(gl.ARRAY_BUFFER, poleMesh, gl.STATIC_DRAW);
      this.uploadedPoleMesh = poleMesh;
    }
    gl.enableVertexAttribArray(this.aPos);
    gl.vertexAttribPointer(this.aPos, 2, gl.FLOAT, false, 0, 0);
    gl.drawArrays(gl.TRIANGLES, 0, poleMesh.length / 2);
  }

  private getPoleMesh(): Float32Array {
    const [x, y, w, h, z] = getViewportTileRange(this.map, true);
    const worldTiles = Math.pow(2, z);
    // Match MapLibre's globe tile subdivision closely enough to keep the
    // Mercator boundary smooth, without depending on its private internals.
    const granularity = Math.max(Math.floor(128 / worldTiles), 32);
    const key = `${x}/${y}/${w}/${h}/${z}/${granularity}`;
    if (key === this.meshKey) return this.poleMesh;

    const drawNorthPole = y === 0;
    const drawSouthPole = y + h === worldTiles;
    const vertices: number[] = [];
    const step = 1 / worldTiles / granularity;

    const appendPole = (poleY: number, edgeY: number): void => {
      for (let tileX = x; tileX < x + w; tileX++) {
        const offset = tileX / worldTiles;
        for (let segment = 0; segment < granularity; segment++) {
          const low = offset + segment * step;
          const high = low + step;
          vertices.push(low, edgeY, high, edgeY, (low + high) / 2, poleY);
        }
      }
    };

    if (drawNorthPole) appendPole(NORTH_POLE_Y, 0);
    if (drawSouthPole) appendPole(SOUTH_POLE_Y, 1);

    this.meshKey = key;
    this.poleMesh = new Float32Array(vertices);
    return this.poleMesh;
  }

  private setupShader(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    args: CustomRenderMethodInput,
  ): void {
    const vertexSource = `#version 300 es
      ${args.shaderData.vertexShaderPrelude}
      ${args.shaderData.define}
      in vec2 a_pos;
      void main() {
        gl_Position = projectTile(a_pos, a_pos);
      }`;
    const fragmentSource = `#version 300 es
      precision mediump float;
      uniform vec4 u_bgColor;
      out highp vec4 fragColor;
      void main() {
        fragColor = u_bgColor;
      }`;

    const vertexShader = this.compileShader(gl, gl.VERTEX_SHADER, vertexSource);
    const fragmentShader = this.compileShader(
      gl,
      gl.FRAGMENT_SHADER,
      fragmentSource,
    );
    if (!vertexShader || !fragmentShader) return;

    const program = gl.createProgram();
    if (!program) return;
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);
    gl.deleteShader(vertexShader);
    gl.deleteShader(fragmentShader);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error(
        "CanvasPoleFoggyLayer program link failed:",
        gl.getProgramInfoLog(program),
      );
      gl.deleteProgram(program);
      return;
    }

    this.program = program;
    this.aPos = gl.getAttribLocation(program, "a_pos");
    this.uBgColor = gl.getUniformLocation(program, "u_bgColor");
    this.projectionUniforms = {
      clippingPlane: gl.getUniformLocation(
        program,
        "u_projection_clipping_plane",
      ),
      transition: gl.getUniformLocation(program, "u_projection_transition"),
      tileMercatorCoords: gl.getUniformLocation(
        program,
        "u_projection_tile_mercator_coords",
      ),
      matrix: gl.getUniformLocation(program, "u_projection_matrix"),
      fallbackMatrix: gl.getUniformLocation(
        program,
        "u_projection_fallback_matrix",
      ),
    };
    this.buffer = gl.createBuffer();
  }

  private compileShader(
    gl: WebGLRenderingContext | WebGL2RenderingContext,
    type: number,
    source: string,
  ): WebGLShader | null {
    const shader = gl.createShader(type);
    if (!shader) return null;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (gl.getShaderParameter(shader, gl.COMPILE_STATUS)) return shader;

    console.error(
      "CanvasPoleFoggyLayer shader compilation failed:",
      gl.getShaderInfoLog(shader),
    );
    gl.deleteShader(shader);
    return null;
  }
}
