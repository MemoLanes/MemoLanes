import type { Map, MapMovementEvent } from "maplibre-gl";

/** Observe intentional single taps without interfering with map gestures. */
export function observeSingleMapTap(
  map: Pick<Map, "getCanvas" | "on" | "off">,
  onTap: () => void,
): () => void {
  const canvas = map.getCanvas();
  const pointers = new Set<number>();
  let press: PointerEvent | undefined;
  let pending: ReturnType<typeof setTimeout> | undefined;

  const cancelPending = () => {
    clearTimeout(pending);
    pending = undefined;
  };
  const down = (event: PointerEvent) => {
    const secondTap = pending !== undefined;
    cancelPending();
    pointers.add(event.pointerId);
    press =
      !secondTap && pointers.size === 1 && event.button === 0
        ? event
        : undefined;
  };
  const move = (event: PointerEvent) => {
    // Match MapLibre's default 3px drag threshold. A small pan must not also
    // toggle the record controls when the pointer is released.
    if (
      press?.pointerId === event.pointerId &&
      Math.hypot(
        event.clientX - press.clientX,
        event.clientY - press.clientY,
      ) >= 3
    ) {
      press = undefined;
    }
  };
  const up = (event: PointerEvent) => {
    move(event);
    if (
      press?.pointerId === event.pointerId &&
      event.timeStamp - press.timeStamp <= 300
    ) {
      // MapLibre's TapRecognizer gives the second tap 500 ms.
      pending = setTimeout(() => {
        pending = undefined;
        onTap();
      }, 500);
    }
    pointers.delete(event.pointerId);
    press = undefined;
  };
  const cancel = () => {
    press = undefined;
    pointers.clear();
    cancelPending();
  };
  const leave = (event: PointerEvent) => {
    // Touch pointers leave automatically after pointerup; keep that tap pending.
    if (pointers.has(event.pointerId)) cancel();
  };
  const onMoveStart = ({ originalEvent }: MapMovementEvent) => {
    // A double tap at the zoom limit still starts a move.
    const type = originalEvent?.type;
    if (type === "touchend" || type === "dblclick") cancel();
  };
  canvas.addEventListener("pointerdown", down);
  canvas.addEventListener("pointermove", move);
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", cancel);
  canvas.addEventListener("pointerleave", leave);
  window.addEventListener("blur", cancel);
  map.on("zoomstart", cancel);
  map.on("movestart", onMoveStart);
  return () => {
    cancel();
    canvas.removeEventListener("pointerdown", down);
    canvas.removeEventListener("pointermove", move);
    canvas.removeEventListener("pointerup", up);
    canvas.removeEventListener("pointercancel", cancel);
    canvas.removeEventListener("pointerleave", leave);
    window.removeEventListener("blur", cancel);
    map.off("zoomstart", cancel);
    map.off("movestart", onMoveStart);
  };
}
