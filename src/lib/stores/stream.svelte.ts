import { startAudio, stopAudio } from '$lib/audio/context';
import { buildConstraints } from '$lib/capture/constraints';
import { logger } from '$lib/logger';

export type StreamStatus = 'idle' | 'acquiring' | 'active' | 'error';

export interface StreamError {
  /** Short identifier the UI can branch on for wording. */
  kind: 'permission' | 'not-found' | 'in-use' | 'disconnected' | 'unknown';
  /** Human-readable message for the error overlay. */
  message: string;
}

function classifyGetUserMediaError(err: unknown): StreamError {
  if (err instanceof DOMException) {
    switch (err.name) {
      case 'NotAllowedError':
      case 'SecurityError':
        return {
          kind: 'permission',
          message:
            "Beamview can't access your camera. Open System Settings > Privacy & Security > Camera to allow.",
        };
      case 'NotFoundError':
      case 'OverconstrainedError':
        return {
          kind: 'not-found',
          message: 'The selected device is no longer available.',
        };
      case 'NotReadableError':
      case 'AbortError':
        return {
          kind: 'in-use',
          message: 'The capture device is busy. Close other apps using it and try again.',
        };
      default:
        return { kind: 'unknown', message: err.message };
    }
  }
  return {
    kind: 'unknown',
    message: err instanceof Error ? err.message : String(err),
  };
}

class StreamStore {
  value = $state<MediaStream | null>(null);
  status = $state<StreamStatus>('idle');
  error = $state<StreamError | null>(null);
  currentVideoId = $state<string | null>(null);
  currentAudioId = $state<string | null>(null);

  async acquire(videoDeviceId: string, audioDeviceId: string | null): Promise<void> {
    this.release();
    this.status = 'acquiring';
    this.error = null;

    try {
      const constraints = buildConstraints({ videoDeviceId, audioDeviceId });
      logger.debug('acquiring MediaStream', {
        video: videoDeviceId,
        audio: audioDeviceId,
      });
      const stream = await navigator.mediaDevices.getUserMedia(constraints);

      this.value = stream;
      this.currentVideoId = videoDeviceId;
      this.currentAudioId = audioDeviceId;

      if (audioDeviceId) startAudio(stream);

      // Listen once on every track so a single disconnect flips status.
      stream.getTracks().forEach((track) => {
        track.addEventListener('ended', () => this.handleTrackEnded(), { once: true });
      });

      this.status = 'active';
      logger.info('stream active', {
        video: videoDeviceId,
        audio: audioDeviceId,
        tracks: stream.getTracks().length,
      });
    } catch (err) {
      this.error = classifyGetUserMediaError(err);
      this.status = 'error';
      logger.warn('stream acquisition failed', {
        kind: this.error.kind,
        message: this.error.message,
      });
    }
  }

  release(): void {
    if (this.value) {
      // Must stop every track — clearing srcObject alone keeps the
      // hardware reserved and the device appears "busy" to other apps.
      this.value.getTracks().forEach((t) => t.stop());
      this.value = null;
    }
    stopAudio();
    this.currentVideoId = null;
    this.currentAudioId = null;
    this.status = 'idle';
  }

  private handleTrackEnded(): void {
    // Fired when the capture card is unplugged mid-stream.
    const hadStream = this.value !== null;
    this.release();
    if (hadStream) {
      this.error = {
        kind: 'disconnected',
        message: 'The capture device was disconnected. Plug it back in and refresh.',
      };
      this.status = 'error';
      logger.warn('stream ended — device disconnected');
    }
  }
}

export const stream = new StreamStore();
