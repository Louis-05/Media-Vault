<script lang="ts">
  import type { MediaInfo } from "../api";
  import { getMediaThumbnail, getMediaPath, getThumbnailPath, getPreviewData, getPreviewFilePath, toMediaUrl, revealMedia, copyMediaFile, copyMediaPath, deleteMedia } from "../api";
  import { startDrag } from "@crabnebula/tauri-plugin-drag";
  import { isDraggingOut, previewVolume } from "../stores/vault";
  import { onMount } from "svelte";

  interface Props {
    media: MediaInfo;
    score?: number;
    onMediaDeleted: () => void;
    onEditDescription?: (mediaId: string) => void;
  }
  let { media, score, onMediaDeleted, onEditDescription }: Props = $props();

  let thumbnailSrc = $state<string | null>(null);
  let previewGifSrc = $state<string | null>(null);
  let previewVideoUrl = $state<string | null>(null);
  let audioUrl = $state<string | null>(null);
  let audioEl: HTMLAudioElement | null = null;
  let previewIsVideo = $state(false);
  let videoEl = $state<HTMLVideoElement | null>(null);
  let previewKey = $state(0);
  let hovering = $state(false);
  let showMenu = $state(false);
  let confirmDelete = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);

  let hasAnimatedPreview = $derived(media.media_type === "video" || media.media_type === "gif");
  let isAudio = $derived(media.media_type === "audio");
  let cardEl: HTMLDivElement | undefined;

  // When the grid reshuffles (search, new items), the card can move out from under
  // the cursor without triggering mouseleave. Detect this and stop playback.
  $effect(() => {
    if (!hovering) return;
    const interval = setInterval(() => {
      if (!cardEl) return;
      const rect = cardEl.getBoundingClientRect();
      // Check if a pointer is still inside (no API for this, so check via :hover)
      if (!cardEl.matches(":hover")) {
        hovering = false;
        stopAudio();
      }
    }, 200);
    return () => clearInterval(interval);
  });

  function scoreColor(s: number): string {
    const hue = Math.round(s * 120);
    return `hsl(${hue}, 70%, 45%)`;
  }

  function formatDuration(seconds: number): string {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
    return `${m}:${String(s).padStart(2, "0")}`;
  }

  onMount(async () => {
    try {
      const b64 = await getMediaThumbnail(media.id);
      thumbnailSrc = `data:image/webp;base64,${b64}`;
    } catch {
      thumbnailSrc = null;
    }

    // Load animated preview
    if (media.media_type === "video" || media.media_type === "gif") {
      try {
        const previewPath = await getPreviewFilePath(media.id);
        if (previewPath && previewPath.endsWith(".mp4")) {
          previewVideoUrl = toMediaUrl(previewPath);
          previewIsVideo = true;
        } else {
          // Fallback to base64 GIF
          const b64 = await getPreviewData(media.id);
          if (b64) previewGifSrc = `data:image/gif;base64,${b64}`;
        }
      } catch {
        // Try legacy GIF fallback
        try {
          const b64 = await getPreviewData(media.id);
          if (b64) previewGifSrc = `data:image/gif;base64,${b64}`;
        } catch { /* no preview */ }
      }
    }

    // Load audio URL for audio files
    if (media.media_type === "audio") {
      try {
        const path = await getMediaPath(media.id);
        audioUrl = toMediaUrl(path);
      } catch { /* no audio */ }
    }

    function closeMenu() {
      showMenu = false;
      confirmDelete = false;
    }
    window.addEventListener("click", closeMenu);
    return () => {
      window.removeEventListener("click", closeMenu);
      stopAudio();
    };
  });

  function playAudio() {
    if (!audioUrl) return;
    const vol = $previewVolume;
    if (vol === 0) return;
    audioEl = new Audio(audioUrl);
    audioEl.volume = vol;
    audioEl.play().catch(() => {});
  }

  function stopAudio() {
    if (audioEl) {
      audioEl.pause();
      audioEl.currentTime = 0;
      audioEl = null;
    }
  }

  async function handleDragStart(e: MouseEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    hovering = false;
    stopAudio();
    try {
      const path = await getMediaPath(media.id);
      const icon = await getThumbnailPath(media.id);
      isDraggingOut.set(true);
      await startDrag({ item: [path], icon });
      setTimeout(() => isDraggingOut.set(false), 500);
    } catch (err) {
      isDraggingOut.set(false);
      console.error("Drag failed:", err);
    }
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    menuX = e.clientX;
    menuY = e.clientY;
    showMenu = true;
    confirmDelete = false;
  }

  async function handleReveal() {
    showMenu = false;
    try {
      await revealMedia(media.id);
    } catch (err) {
      console.error("Reveal failed:", err);
    }
  }

  async function handleCopyFile() {
    showMenu = false;
    try {
      await copyMediaFile(media.id);
    } catch (err) {
      console.error("Copy failed:", err);
    }
  }

  async function handleCopyPath() {
    showMenu = false;
    try {
      await copyMediaPath(media.id);
    } catch (err) {
      console.error("Copy path failed:", err);
    }
  }

  function handleEditDescription() {
    showMenu = false;
    if (onEditDescription) {
      onEditDescription(media.id);
    }
  }

  $effect(() => {
    const vol = $previewVolume;
    if (videoEl) {
      videoEl.volume = vol;
      videoEl.muted = vol === 0;
    }
    if (audioEl) {
      audioEl.volume = vol;
      if (vol === 0) { audioEl.pause(); } else if (hovering && isAudio) { audioEl.play().catch(() => {}); }
    }
  });

  function handleDeleteClick() {
    confirmDelete = true;
  }

  async function handleDeleteConfirm() {
    showMenu = false;
    confirmDelete = false;
    try {
      await deleteMedia(media.id);
      onMediaDeleted();
    } catch (err) {
      console.error("Delete failed:", err);
    }
  }

  function handleDeleteCancel() {
    confirmDelete = false;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="card"
  bind:this={cardEl}
  onmousedown={handleDragStart}
  oncontextmenu={handleContextMenu}
  onmouseenter={() => { hovering = true; previewKey++; if (isAudio) playAudio(); }}
  onmouseleave={() => { hovering = false; if (isAudio) stopAudio(); }}
>
  <div class="thumbnail">
    {#if hovering && previewIsVideo && previewVideoUrl && hasAnimatedPreview}
      {#key previewKey}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          src={previewVideoUrl}
          autoplay
          loop
          volume={$previewVolume}
          muted={$previewVolume === 0}
          bind:this={videoEl}
          onloadedmetadata={() => { if (videoEl) videoEl.volume = $previewVolume; }}
          class="preview-img"
        ></video>
      {/key}
    {:else if hovering && previewGifSrc && hasAnimatedPreview}
      {#key previewKey}
        <img src={previewGifSrc} alt={media.id} draggable="false" class="preview-img" />
      {/key}
    {:else if isAudio}
      <div class="placeholder" class:audio-playing={hovering && audioUrl}>
        <span>🎵</span>
      </div>
    {:else if thumbnailSrc}
      <img src={thumbnailSrc} alt={media.id} draggable="false" />
    {:else}
      <div class="placeholder">
        <span>{media.media_type === "video" ? "🎬" : "🖼"}</span>
      </div>
    {/if}
    {#if score !== undefined}
      <span class="score" style="background-color: {scoreColor(score)}">{Math.round(score * 100)}%</span>
    {/if}
    {#if media.duration}
      <span class="duration">{formatDuration(media.duration)}</span>
    {/if}
    <span class="badge">{media.codec ? `${media.extension}/${media.codec}` : media.extension}</span>
  </div>
  <div class="info">
    {#if media.description}
      <span class="desc" title={media.description}>{media.description}</span>
    {:else}
      <span class="no-desc">No description</span>
    {/if}
  </div>
</div>

{#if showMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="context-menu" style="left: {menuX}px; top: {menuY}px;" onclick={(e) => e.stopPropagation()}>
    <button onclick={handleEditDescription}>Edit description</button>
    <button onclick={handleReveal}>Reveal in file explorer</button>
    <button onclick={handleCopyFile}>Copy media</button>
    <button onclick={handleCopyPath}>Copy path</button>
    <div class="separator"></div>
    {#if confirmDelete}
      <div class="delete-confirm">
        <span>Are you sure?</span>
        <div class="delete-actions">
          <button class="delete-yes" onclick={handleDeleteConfirm}>Delete</button>
          <button class="delete-no" onclick={handleDeleteCancel}>Cancel</button>
        </div>
      </div>
    {:else}
      <button class="delete-btn" onclick={handleDeleteClick}>Delete</button>
    {/if}
  </div>
{/if}

<style>
  .card {
    background-color: var(--bg-card);
    border-radius: 8px;
    overflow: hidden;
    transition: transform 0.15s, box-shadow 0.15s;
    cursor: grab;
    user-select: none;
  }

  .card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px var(--shadow);
  }

  .card:active {
    cursor: grabbing;
  }

  .thumbnail {
    position: relative;
    aspect-ratio: 1;
    background-color: var(--bg);
    overflow: hidden;
  }

  .thumbnail img,
  .thumbnail video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
  }

  .placeholder {
    font-size: 3rem;
    opacity: 0.3;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    transition: opacity 0.2s;
  }

  .placeholder.audio-playing {
    opacity: 0.7;
  }

  .score {
    position: absolute;
    top: 6px;
    left: 6px;
    color: white;
    font-size: 0.7rem;
    font-weight: bold;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .duration {
    position: absolute;
    bottom: 6px;
    right: 6px;
    background-color: rgba(0, 0, 0, 0.7);
    color: white;
    font-size: 0.65rem;
    font-weight: bold;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .badge {
    position: absolute;
    top: 6px;
    right: 6px;
    background-color: rgba(0, 0, 0, 0.7);
    color: white;
    font-size: 0.65rem;
    font-weight: bold;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .info {
    padding: 8px 10px;
  }

  .desc {
    font-size: 0.75rem;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.3;
  }

  .no-desc {
    font-size: 0.7rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .context-menu {
    position: fixed;
    z-index: 1000;
    background-color: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 0;
    box-shadow: 0 8px 24px var(--shadow);
    min-width: 180px;
  }

  .context-menu button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 8px 14px;
    background: none;
    color: var(--text);
    font-size: 0.85rem;
    border-radius: 0;
  }

  .context-menu button:hover {
    background-color: var(--border);
  }

  .separator {
    height: 1px;
    background-color: var(--border);
    margin: 4px 0;
  }

  .delete-btn {
    color: #ff4444 !important;
  }

  .delete-btn:hover {
    background-color: rgba(255, 68, 68, 0.15) !important;
  }

  .delete-confirm {
    padding: 8px 14px;
  }

  .delete-confirm span {
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .delete-actions {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }

  .delete-yes {
    flex: 1;
    background-color: #ff4444 !important;
    color: white !important;
    text-align: center !important;
    border-radius: 4px !important;
    padding: 4px 8px !important;
    font-size: 0.8rem !important;
  }

  .delete-yes:hover {
    background-color: #cc3333 !important;
  }

  .delete-no {
    flex: 1;
    text-align: center !important;
    border: 1px solid var(--border) !important;
    border-radius: 4px !important;
    padding: 4px 8px !important;
    font-size: 0.8rem !important;
  }
</style>
