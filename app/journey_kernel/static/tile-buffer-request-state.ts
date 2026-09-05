import type { TileRange } from "./layers/tile-range";

/** The request-relevant part of the latest viewport/data demand. */
export interface TileBufferDemand {
  mapMoving: boolean;
  zoomingOut: boolean;
  activeRequestMatchesResolution: boolean;
  activeRequestCoversDesiredRange: boolean;
  activeRequestOverlapsDesiredRange: boolean;
}

/** One network request, including the newest demand it may satisfy. */
export interface ActiveTileBufferRequest {
  requestId: number;
  range: TileRange;
  bufferSizePower: number;
  commitThroughRequestId: number | null;
}

/**
 * Request lifecycle state. `desiredRequestId` is a demand revision, while
 * `requestQueued` says whether that revision still needs a network request.
 */
export interface TileBufferRequestState {
  desiredRequestId: number;
  requestQueued: boolean;
  active: ActiveTileBufferRequest | null;
}

export type ActiveRequestDisposition = "preserve" | "supersede";

export function createTileBufferRequestState(): TileBufferRequestState {
  return {
    desiredRequestId: 0,
    requestQueued: false,
    active: null,
  };
}

/**
 * Advance the demand revision. A deferred viewport update can invalidate an
 * active request without immediately queueing its replacement.
 */
export function advanceTileBufferDemand(
  state: TileBufferRequestState,
  options: {
    queueRequest: boolean;
    activeRequest: ActiveRequestDisposition;
  },
): TileBufferRequestState {
  const desiredRequestId = state.desiredRequestId + 1;
  const active = state.active
    ? {
        ...state.active,
        commitThroughRequestId:
          options.activeRequest === "preserve" ? desiredRequestId : null,
      }
    : null;

  return {
    desiredRequestId,
    requestQueued: state.requestQueued || options.queueRequest,
    active,
  };
}

/** Revoke a preserved request at an interaction boundary such as moveend. */
export function revokeActiveTileBufferRequest(
  state: TileBufferRequestState,
): TileBufferRequestState {
  if (!state.active || state.active.commitThroughRequestId === null) {
    return state;
  }
  return {
    ...state,
    active: {
      ...state.active,
      commitThroughRequestId: null,
    },
  };
}

/** Start the newest queued request. Requests are intentionally serialized. */
export function startTileBufferRequest(
  state: TileBufferRequestState,
  range: TileRange,
  bufferSizePower: number,
): {
  state: TileBufferRequestState;
  active: ActiveTileBufferRequest;
} {
  if (state.active) {
    throw new Error("A tile buffer request is already active");
  }
  if (!state.requestQueued) {
    throw new Error("No tile buffer request is queued");
  }

  const active: ActiveTileBufferRequest = {
    requestId: state.desiredRequestId,
    range,
    bufferSizePower,
    commitThroughRequestId: state.desiredRequestId,
  };
  return {
    active,
    state: {
      ...state,
      requestQueued: false,
      active,
    },
  };
}

/** Clear only the request that actually settled; newer state is untouched. */
export function settleTileBufferRequest(
  state: TileBufferRequestState,
  requestId: number,
): TileBufferRequestState {
  if (state.active?.requestId !== requestId) {
    return state;
  }
  return {
    ...state,
    active: null,
  };
}

/** Whether the active request is still useful enough to preserve in flight. */
export function shouldPreserveActiveTileBufferRequest(
  state: TileBufferRequestState,
  demand: TileBufferDemand,
): boolean {
  const active = state.active;
  if (!active || !demand.mapMoving) return false;
  if (!demand.activeRequestMatchesResolution) {
    return false;
  }

  return (
    demand.activeRequestCoversDesiredRange ||
    (demand.zoomingOut && demand.activeRequestOverlapsDesiredRange)
  );
}

/**
 * The response may commit only when it is permitted for the exact latest
 * demand revision and remains spatially useful at commit time.
 */
export function canCommitActiveTileBufferRequest(
  state: TileBufferRequestState,
  requestId: number,
  demand: TileBufferDemand,
): boolean {
  const active = state.active;
  if (
    !active ||
    active.requestId !== requestId ||
    active.commitThroughRequestId !== state.desiredRequestId
  ) {
    return false;
  }

  if (active.requestId === state.desiredRequestId) {
    return (
      demand.activeRequestMatchesResolution &&
      demand.activeRequestCoversDesiredRange
    );
  }

  return shouldPreserveActiveTileBufferRequest(state, demand);
}
