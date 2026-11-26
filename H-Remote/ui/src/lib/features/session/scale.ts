/**
 * Converts CSS viewport units into physical device pixels.
 *
 * RDP accepts the remote desktop size in pixels, while DOM measurements are in
 * CSS pixels. Sending CSS pixels directly made every DPR > 1 session request a
 * smaller remote desktop than the actual window, so full screen and fit modes
 * appeared to shrink the pixel size.
 */
export function physicalPixels(
  width: number,
  height: number,
  ratio: number,
): { width: number; height: number } {
  const scale = Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
  return {
    width: Math.round(width * scale),
    height: Math.round(height * scale),
  };
}
