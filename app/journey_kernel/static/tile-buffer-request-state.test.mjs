import assert from "node:assert/strict";
import test from "node:test";

import {
  advanceTileBufferDemand,
  canCommitActiveTileBufferRequest,
  createTileBufferRequestState,
  revokeActiveTileBufferRequest,
  settleTileBufferRequest,
  shouldPreserveActiveTileBufferRequest,
  startTileBufferRequest,
} from "./tile-buffer-request-state.ts";

const firstRange = [10, 10, 4, 4, 6];
const expandedRange = [9, 9, 6, 6, 6];

function queue(state, activeRequest = "supersede") {
  return advanceTileBufferDemand(state, {
    queueRequest: true,
    activeRequest,
  });
}

function demand({
  mapMoving = false,
  zoomingOut = false,
  matchesResolution = true,
  coversRange = true,
  overlapsRange = true,
} = {}) {
  return {
    mapMoving,
    zoomingOut,
    activeRequestMatchesResolution: matchesResolution,
    activeRequestCoversDesiredRange: coversRange,
    activeRequestOverlapsDesiredRange: overlapsRange,
  };
}

test("a response for the exact latest demand can commit", () => {
  const queued = queue(createTileBufferRequestState());
  const { state, active } = startTileBufferRequest(queued, firstRange, 10);

  assert.equal(
    canCommitActiveTileBufferRequest(state, active.requestId, demand()),
    true,
  );
});

test("continuous zoom-out grants an old request one explicit commit permit", () => {
  const queued = queue(createTileBufferRequestState());
  let { state, active } = startTileBufferRequest(queued, firstRange, 10);
  const nextDemand = demand({
    mapMoving: true,
    zoomingOut: true,
    coversRange: false,
  });

  assert.equal(shouldPreserveActiveTileBufferRequest(state, nextDemand), true);
  state = queue(state, "preserve");
  assert.equal(state.requestQueued, true);
  assert.equal(
    canCommitActiveTileBufferRequest(state, active.requestId, nextDemand),
    true,
  );

  const latestDemand = demand({
    mapMoving: true,
    zoomingOut: true,
    coversRange: false,
  });
  state = queue(state, "preserve");
  assert.equal(
    canCommitActiveTileBufferRequest(state, active.requestId, latestDemand),
    true,
    "each compatible viewport revision explicitly renews the permit",
  );
});

test("moveend rejects a preserved response that arrives afterward", () => {
  const queued = queue(createTileBufferRequestState());
  let { state, active } = startTileBufferRequest(queued, firstRange, 10);
  state = queue(state, "preserve");
  state = revokeActiveTileBufferRequest(state);

  assert.equal(
    canCommitActiveTileBufferRequest(state, active.requestId, demand()),
    false,
  );
});

test("an explicit refresh cannot be satisfied by an older viewport response", () => {
  const queued = queue(createTileBufferRequestState());
  let { state, active } = startTileBufferRequest(queued, firstRange, 10);
  state = queue(state, "supersede");

  assert.equal(
    canCommitActiveTileBufferRequest(state, active.requestId, demand()),
    false,
  );
});

test("a changed resolution or disjoint viewport cannot preserve a request", () => {
  const queued = queue(createTileBufferRequestState());
  const { state } = startTileBufferRequest(queued, firstRange, 10);
  assert.equal(
    shouldPreserveActiveTileBufferRequest(
      state,
      demand({
        mapMoving: true,
        zoomingOut: true,
        matchesResolution: false,
      }),
    ),
    false,
  );
  assert.equal(
    shouldPreserveActiveTileBufferRequest(
      state,
      demand({
        mapMoving: true,
        zoomingOut: true,
        coversRange: false,
        overlapsRange: false,
      }),
    ),
    false,
  );
});

test("settling old work retains the coalesced latest request", () => {
  const queued = queue(createTileBufferRequestState());
  let { state, active } = startTileBufferRequest(queued, firstRange, 10);
  state = queue(state, "preserve");
  state = settleTileBufferRequest(state, active.requestId);

  assert.equal(state.active, null);
  assert.equal(state.requestQueued, true);
  assert.equal(state.desiredRequestId, 2);

  const next = startTileBufferRequest(state, expandedRange, 10);
  assert.equal(next.active.requestId, 2);
  assert.equal(next.state.requestQueued, false);
});

test("deferred viewport demand invalidates work without queueing a fetch", () => {
  const queued = queue(createTileBufferRequestState());
  let { state, active } = startTileBufferRequest(queued, firstRange, 10);
  state = advanceTileBufferDemand(state, {
    queueRequest: false,
    activeRequest: "supersede",
  });

  assert.equal(state.requestQueued, false);
  assert.equal(
    canCommitActiveTileBufferRequest(state, active.requestId, demand()),
    false,
  );
});
