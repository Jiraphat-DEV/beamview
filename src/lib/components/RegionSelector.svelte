<script lang="ts">
  // RegionSelector — M4
  //
  // Full-viewport overlay that lets the user drag a rectangle over a
  // freeze-frame of the live stream to define the subtitle OCR region.
  //
  // ── Coordinate spaces ────────────────────────────────────────────────────
  //
  // The rectangle the user drags is in *screen pixel* space (DOM).
  // The region we persist must be in *video native* space so it matches what
  // Apple Vision receives when the frame sampler crops the JPEG.
  //
  // Conversion on Save:
  //   scaleX = videoEl.videoWidth  / videoRect.width   (DOM→native)
  //   scaleY = videoEl.videoHeight / videoRect.height
  //   nativeX     = (screenX - videoRect.left) * scaleX
  //   nativeY     = (screenY - videoRect.top)  * scaleY
  //   nativeW     = screenW * scaleX
  //   nativeH     = screenH * scaleY
  //
  // The result is clamped to [0, videoWidth] × [0, videoHeight] and has a
  // minimum size of 40×20 px in native space.

  import { onMount } from 'svelte';
  import { translation } from '$lib/stores/translation.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import type { Region } from '$lib/ipc/commands';

  const MODAL_ID = 'region-selector';
  const MIN_W_NATIVE = 40;
  const MIN_H_NATIVE = 20;

  interface Props {
    videoEl: HTMLVideoElement;
  }

  let { videoEl }: Props = $props();

  // The canvas that holds the frozen frame.
  let canvasEl: HTMLCanvasElement | null = $state(null);
  // The overlay container (for pointer-event capture).
  let overlayEl: HTMLDivElement | null = $state(null);

  // Rectangle in *screen* pixels (relative to the canvas/video DOM rect).
  let screenRect = $state({ x: 0, y: 0, w: 0, h: 0 });

  // Cached video DOM rect (updated on mount).
  let videoDomRect = $state<DOMRect | null>(null);

  onMount(() => {
    // Freeze the current video frame onto the canvas.
    if (canvasEl && videoEl) {
      const vr = videoEl.getBoundingClientRect();
      videoDomRect = vr;

      canvasEl.width = vr.width;
      canvasEl.height = vr.height;

      const ctx = canvasEl.getContext('2d');
      if (ctx) {
        ctx.drawImage(videoEl, 0, 0, vr.width, vr.height);
      }

      // Initialise rectangle from the persisted region (if any), else default
      // to the lower-third of the frame.
      const existing = translation.region;
      if (existing) {
        // Convert from native → screen coordinates.
        const sx = vr.width / videoEl.videoWidth;
        const sy = vr.height / videoEl.videoHeight;
        screenRect = {
          x: existing.x * sx,
          y: existing.y * sy,
          w: existing.width * sx,
          h: existing.height * sy,
        };
      } else {
        screenRect = {
          x: 0,
          y: vr.height * 0.7,
          w: vr.width,
          h: vr.height * 0.3,
        };
      }
    }

    // Handle Esc / Enter at the overlay level.
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') handleCancel();
      if (e.key === 'Enter') handleSave();
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  // ── Drag/resize state ─────────────────────────────────────────────────────

  type Handle = 'move' | 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

  let dragHandle = $state<Handle | null>(null);
  let dragStart = { x: 0, y: 0 };
  let rectAtDragStart = { x: 0, y: 0, w: 0, h: 0 };

  function startDrag(e: PointerEvent, handle: Handle) {
    e.preventDefault();
    dragHandle = handle;
    dragStart = { x: e.clientX, y: e.clientY };
    rectAtDragStart = { ...screenRect };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragHandle || !videoDomRect) return;

    const dx = e.clientX - dragStart.x;
    const dy = e.clientY - dragStart.y;
    const { x: ox, y: oy, w: ow, h: oh } = rectAtDragStart;
    const VW = videoDomRect.width;
    const VH = videoDomRect.height;

    let nx = ox,
      ny = oy,
      nw = ow,
      nh = oh;

    if (dragHandle === 'move') {
      nx = ox + dx;
      ny = oy + dy;
    } else {
      // Resize — only the relevant edges move.
      if (dragHandle.includes('e')) nw = ow + dx;
      if (dragHandle.includes('s')) nh = oh + dy;
      if (dragHandle.includes('w')) {
        nx = ox + dx;
        nw = ow - dx;
      }
      if (dragHandle.includes('n')) {
        ny = oy + dy;
        nh = oh - dy;
      }
    }

    // Enforce minimum size.
    const minWScreen =
      videoDomRect.width > 0 ? (MIN_W_NATIVE / videoEl.videoWidth) * videoDomRect.width : 40;
    const minHScreen =
      videoDomRect.height > 0 ? (MIN_H_NATIVE / videoEl.videoHeight) * videoDomRect.height : 20;
    if (nw < minWScreen) {
      if (dragHandle.includes('w')) nx = nx - (minWScreen - nw);
      nw = minWScreen;
    }
    if (nh < minHScreen) {
      if (dragHandle.includes('n')) ny = ny - (minHScreen - nh);
      nh = minHScreen;
    }

    // Clamp to video bounds.
    nx = Math.max(0, Math.min(VW - nw, nx));
    ny = Math.max(0, Math.min(VH - nh, ny));
    nw = Math.min(nw, VW - nx);
    nh = Math.min(nh, VH - ny);

    screenRect = { x: nx, y: ny, w: nw, h: nh };
  }

  function onPointerUp() {
    dragHandle = null;
  }

  // ── Save / Cancel ─────────────────────────────────────────────────────────

  function handleSave() {
    if (!videoDomRect || !videoEl) return;

    const scaleX = videoEl.videoWidth / videoDomRect.width;
    const scaleY = videoEl.videoHeight / videoDomRect.height;

    const region: Region = {
      x: Math.round(screenRect.x * scaleX),
      y: Math.round(screenRect.y * scaleY),
      width: Math.round(screenRect.w * scaleX),
      height: Math.round(screenRect.h * scaleY),
    };

    // Clamp to native bounds and enforce minimum.
    region.x = Math.max(0, Math.min(videoEl.videoWidth - MIN_W_NATIVE, region.x));
    region.y = Math.max(0, Math.min(videoEl.videoHeight - MIN_H_NATIVE, region.y));
    region.width = Math.max(MIN_W_NATIVE, Math.min(videoEl.videoWidth - region.x, region.width));
    region.height = Math.max(MIN_H_NATIVE, Math.min(videoEl.videoHeight - region.y, region.height));

    translation.setRegion(region);
    ui.popModal(MODAL_ID);
  }

  function handleCancel() {
    ui.popModal(MODAL_ID);
  }

  // ── Derived handle positions (in screen space) ─────────────────────────────

  const handles: Array<{ id: Handle; cx: number; cy: number }> = $derived.by(() => {
    const { x, y, w, h } = screenRect;
    return [
      { id: 'nw', cx: x, cy: y },
      { id: 'n', cx: x + w / 2, cy: y },
      { id: 'ne', cx: x + w, cy: y },
      { id: 'e', cx: x + w, cy: y + h / 2 },
      { id: 'se', cx: x + w, cy: y + h },
      { id: 's', cx: x + w / 2, cy: y + h },
      { id: 'sw', cx: x, cy: y + h },
      { id: 'w', cx: x, cy: y + h / 2 },
    ];
  });

  const cursorForHandle: Record<Handle, string> = {
    move: 'move',
    nw: 'nw-resize',
    n: 'n-resize',
    ne: 'ne-resize',
    e: 'e-resize',
    se: 'se-resize',
    s: 's-resize',
    sw: 'sw-resize',
    w: 'w-resize',
  };
</script>

<div
  class="overlay"
  bind:this={overlayEl}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  role="presentation"
>
  <!-- Frozen frame background -->
  <canvas bind:this={canvasEl} class="freeze-frame"></canvas>

  <!-- Dark vignette outside the selection region -->
  <svg class="vignette" aria-hidden="true">
    <defs>
      <mask id="cutout">
        <rect width="100%" height="100%" fill="white" />
        <rect
          x={screenRect.x}
          y={screenRect.y}
          width={screenRect.w}
          height={screenRect.h}
          fill="black"
        />
      </mask>
    </defs>
    <rect width="100%" height="100%" fill="rgba(0,0,0,0.55)" mask="url(#cutout)" />
  </svg>

  <!-- Draggable rectangle border -->
  <div
    class="selection"
    style="left:{screenRect.x}px; top:{screenRect.y}px; width:{screenRect.w}px; height:{screenRect.h}px;"
    onpointerdown={(e) => startDrag(e, 'move')}
    role="presentation"
    style:cursor={dragHandle ? cursorForHandle[dragHandle] : 'move'}
  >
    <!-- 8 resize handles -->
    {#each handles as h (h.id)}
      <div
        class="handle"
        style="left:{h.cx - screenRect.x - 5}px; top:{h.cy - screenRect.y - 5}px;"
        style:cursor={cursorForHandle[h.id]}
        onpointerdown={(e) => {
          e.stopPropagation();
          startDrag(e, h.id);
        }}
        role="presentation"
      ></div>
    {/each}
  </div>

  <!-- Hint label -->
  <div class="hint">ลากกรอบให้ครอบ subtitle</div>

  <!-- Action buttons -->
  <div class="actions">
    <button type="button" class="btn ghost" onclick={handleCancel}>ยกเลิก</button>
    <button type="button" class="btn accent" onclick={handleSave}>บันทึก</button>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 150;
    user-select: none;
    overflow: hidden;
  }

  .freeze-frame {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  .vignette {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .selection {
    position: absolute;
    border: 2px solid rgba(250, 249, 246, 0.9);
    box-sizing: border-box;
    cursor: move;
  }

  .handle {
    position: absolute;
    width: 10px;
    height: 10px;
    background: var(--bv-paper, #faf9f6);
    border: 1px solid rgba(0, 0, 0, 0.4);
    border-radius: 2px;
    box-sizing: border-box;
  }

  .hint {
    position: absolute;
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(26, 26, 26, 0.75);
    color: #faf9f6;
    font-size: 13px;
    padding: 6px 14px;
    border-radius: 4px;
    pointer-events: none;
    white-space: nowrap;
  }

  .actions {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 12px;
  }

  .btn {
    padding: 8px 20px;
    border: 1px solid rgba(250, 249, 246, 0.3);
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease);
  }

  .btn.ghost {
    background: rgba(26, 26, 26, 0.65);
    color: #faf9f6;
  }
  .btn.ghost:hover {
    background: rgba(26, 26, 26, 0.85);
  }

  .btn.accent {
    background: var(--bv-accent, #d85a30);
    border-color: var(--bv-accent, #d85a30);
    color: #faf9f6;
  }
  .btn.accent:hover {
    background: color-mix(in srgb, var(--bv-accent, #d85a30) 90%, black);
  }
</style>
