import { PERMISSION_PROBE_CONSTRAINTS } from './constraints';

export interface CaptureDevice {
  deviceId: string;
  label: string;
  groupId: string;
}

export interface EnumeratedDevices {
  video: CaptureDevice[];
  audio: CaptureDevice[];
}

const toCaptureDevice = (d: MediaDeviceInfo): CaptureDevice => ({
  deviceId: d.deviceId,
  label: d.label || `Unnamed (${d.deviceId.slice(0, 8)})`,
  groupId: d.groupId,
});

/** Enumerate video (camera) and audio (microphone) inputs.
 *
 * Spec §17.7 decision A: show all devices, no auto-filter. macOS
 * shows FaceTime + capture cards side by side and the user picks.
 * Label data is empty until the user grants media permission, so we
 * fill in a stable placeholder using the opaque deviceId suffix. */
export async function enumerateCaptureDevices(): Promise<EnumeratedDevices> {
  const all = await navigator.mediaDevices.enumerateDevices();
  return {
    video: all.filter((d) => d.kind === 'videoinput').map(toCaptureDevice),
    audio: all.filter((d) => d.kind === 'audioinput').map(toCaptureDevice),
  };
}

/** One-shot getUserMedia call whose only purpose is to surface the
 *  macOS camera + microphone permission prompts. Tracks are stopped
 *  immediately so no stream is left open.
 *
 *  Returns `true` if permission was granted, `false` otherwise.
 *  Swallows the specific rejection reason so callers can still show
 *  a generic "Open System Settings" message — the actual error is
 *  logged for debugging. */
export async function requestPermission(): Promise<boolean> {
  try {
    const stream = await navigator.mediaDevices.getUserMedia(PERMISSION_PROBE_CONSTRAINTS);
    stream.getTracks().forEach((t) => t.stop());
    return true;
  } catch (err) {
    console.debug('[capture] permission probe rejected:', err);
    return false;
  }
}

/** Pick a preferred device from a list, falling back to the first
 *  entry. Used on startup to restore the last-used device from config. */
export function preferDevice<T extends { deviceId: string }>(
  list: T[],
  preferredId: string | null,
): T | null {
  if (list.length === 0) return null;
  if (preferredId) {
    const match = list.find((d) => d.deviceId === preferredId);
    if (match) return match;
  }
  return list[0] ?? null;
}
