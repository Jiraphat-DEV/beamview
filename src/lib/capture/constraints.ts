// Build MediaStreamConstraints for a capture card device.
//
// CRITICAL (spec §5.3): the three audio processing flags must be `false`.
// WebRTC defaults enable echoCancellation, noiseSuppression, and
// autoGainControl — which destroy game audio (echo cancellation eats
// sub-bass, noise suppression makes voices metallic, AGC ducks loud
// sections). These defaults exist to make Zoom calls sound good, not to
// pass a game soundtrack through untouched.

export interface CaptureConstraintsInput {
  videoDeviceId: string;
  audioDeviceId: string | null;
}

export function buildConstraints({
  videoDeviceId,
  audioDeviceId,
}: CaptureConstraintsInput): MediaStreamConstraints {
  return {
    video: {
      deviceId: { exact: videoDeviceId },
      width: { ideal: 1920 },
      height: { ideal: 1080 },
      frameRate: { ideal: 60 },
    },
    audio: audioDeviceId
      ? {
          deviceId: { exact: audioDeviceId },
          // Must stay `false` — do not change without reading spec §5.3.
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
          sampleRate: { ideal: 48000 },
          channelCount: { ideal: 2 },
        }
      : false,
  };
}

/** Minimal constraints used only to trigger the macOS camera/mic
 *  permission dialog on first run. The resulting tracks are stopped
 *  immediately — nothing is rendered. */
export const PERMISSION_PROBE_CONSTRAINTS: MediaStreamConstraints = {
  video: true,
  audio: true,
};
