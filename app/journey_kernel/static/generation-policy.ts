export type GenerationAdvanceOutcome = "committed" | "unchanged" | "failed";

/**
 * Settle the explicit-refresh token consumed by a tile-buffer request.
 *
 * A newer refresh may arrive while the request is in flight, so an already
 * pending token always wins. The consumed token is retried only after a
 * failure; an authoritative unchanged response consumes it.
 */
export function settleGenerationAdvance(
  pending: boolean,
  consumed: boolean,
  outcome: GenerationAdvanceOutcome,
): boolean {
  return pending || (consumed && outcome === "failed");
}
