// Web Audio pipeline for a capture-card MediaStream.
//
// Why not just let <video> play audio?
//   1. We can control volume with a GainNode later without touching video.
//   2. Future features (visualizer, EQ, mute with ramp) plug in easily.
//   3. Audio lifecycle becomes independent of any particular <video>
//      element re-mounting.
//
// The <video> element is rendered with `muted` so macOS doesn't also
// play the audio track through it — Web Audio is the only sound source.

let ctx: AudioContext | null = null;
let sourceNode: MediaStreamAudioSourceNode | null = null;
let gainNode: GainNode | null = null;

export interface StartAudioOptions {
  /** 0 (silent) .. 1 (full). Default 1. */
  volume?: number;
}

/** Attach a MediaStream's audio tracks to the system output. Safe to call
 *  repeatedly — a previous audio context is torn down first. */
export function startAudio(stream: MediaStream, options: StartAudioOptions = {}): void {
  stopAudio();

  // No audio tracks on the stream (audio was disabled in the picker).
  if (stream.getAudioTracks().length === 0) return;

  ctx = new AudioContext({
    latencyHint: 'interactive',
    sampleRate: 48000,
  });

  sourceNode = ctx.createMediaStreamSource(stream);
  gainNode = ctx.createGain();
  gainNode.gain.value = options.volume ?? 1;

  sourceNode.connect(gainNode);
  gainNode.connect(ctx.destination);
}

export function stopAudio(): void {
  if (sourceNode) {
    sourceNode.disconnect();
    sourceNode = null;
  }
  if (gainNode) {
    gainNode.disconnect();
    gainNode = null;
  }
  if (ctx) {
    void ctx.close();
    ctx = null;
  }
}

/** Set the current playback volume (0..1). No-op if audio isn't running. */
export function setVolume(volume: number): void {
  if (gainNode) {
    gainNode.gain.value = Math.max(0, Math.min(1, volume));
  }
}

/** Hard mute (gain=0) without tearing down the graph. Reverse with setVolume. */
export function setMuted(muted: boolean): void {
  if (gainNode) {
    gainNode.gain.value = muted ? 0 : 1;
  }
}
