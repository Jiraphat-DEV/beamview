import { enumerateCaptureDevices, preferDevice, type CaptureDevice } from '$lib/capture/devices';
import { logger } from '$lib/logger';

class DevicesStore {
  video = $state<CaptureDevice[]>([]);
  audio = $state<CaptureDevice[]>([]);
  videoId = $state<string | null>(null);
  audioId = $state<string | null>(null);
  /** Flag flipped after the first successful refresh() so callers can
   *  tell "empty list" from "haven't enumerated yet". */
  ready = $state(false);
  /** Last error surfaced by enumeration (permission denied, etc.). */
  error = $state<string | null>(null);

  /** Re-enumerate media devices. If the current selection is no longer
   *  present, drop it so the UI can prompt the user to pick again. */
  async refresh(): Promise<void> {
    try {
      const { video, audio } = await enumerateCaptureDevices();
      this.video = video;
      this.audio = audio;

      // Drop stale selections.
      if (this.videoId && !video.some((d) => d.deviceId === this.videoId)) {
        this.videoId = null;
      }
      if (this.audioId && !audio.some((d) => d.deviceId === this.audioId)) {
        this.audioId = null;
      }

      this.error = null;
      this.ready = true;
      logger.info('devices refreshed', { video: video.length, audio: audio.length });
    } catch (err) {
      this.error = err instanceof Error ? err.message : String(err);
      this.ready = true;
      logger.warn('device enumeration failed', { err: this.error });
    }
  }

  /** Preselect based on saved IDs from AppConfig. Falls back to the first
   *  entry so something is always pre-chosen once devices are loaded
   *  (spec §17.1 recommendation B — auto-use last device). */
  restoreSelection(savedVideoId: string | null, savedAudioId: string | null): void {
    const v = preferDevice(this.video, savedVideoId);
    const a = preferDevice(this.audio, savedAudioId);
    this.videoId = v?.deviceId ?? null;
    this.audioId = a?.deviceId ?? null;
  }

  setVideo(id: string | null): void {
    this.videoId = id;
  }
  setAudio(id: string | null): void {
    this.audioId = id;
  }
}

export const devices = new DevicesStore();
