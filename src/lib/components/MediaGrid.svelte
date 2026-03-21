<script lang="ts">
  import type { MediaInfo } from "../api";
  import { getZoomLevel, setZoomLevel } from "../api";
  import MediaCard from "./MediaCard.svelte";
  import { previewVolume } from "../stores/vault";
  import { onMount } from "svelte";

  interface Props {
    items: MediaInfo[];
    scores?: Record<string, number>;
    onMediaDeleted: () => void;
    onEditDescription?: (mediaId: string) => void;
    onLoadMore?: () => void;
    hasMore?: boolean;
  }
  let { items, scores, onMediaDeleted, onEditDescription, onLoadMore, hasMore = false }: Props = $props();

  let sentinelEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (!sentinelEl || !onLoadMore || !hasMore) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting && hasMore && onLoadMore) {
        onLoadMore();
      }
    }, { rootMargin: "200px" });
    observer.observe(sentinelEl);
    return () => observer.disconnect();
  });

  let cardSize = $state(200);
  const MIN_SIZE = 80;
  const MAX_SIZE = 500;
  const STEP = 20;

  onMount(async () => {
    const saved = await getZoomLevel();
    if (saved !== null) cardSize = saved;
  });

  function zoomIn() {
    cardSize = Math.min(cardSize + STEP, MAX_SIZE);
    setZoomLevel(cardSize);
  }

  function zoomOut() {
    cardSize = Math.max(cardSize - STEP, MIN_SIZE);
    setZoomLevel(cardSize);
  }

  function handleVolumeChange(e: Event) {
    previewVolume.set(parseFloat((e.target as HTMLInputElement).value));
  }
</script>

<div class="toolbar">
  <div class="volume-control">
    <span class="volume-icon">{$previewVolume === 0 ? '🔇' : $previewVolume < 0.5 ? '🔉' : '🔊'}</span>
    <input
      type="range"
      min="0"
      max="1"
      step="0.05"
      value={$previewVolume}
      oninput={handleVolumeChange}
      class="volume-slider"
      title="Preview volume"
    />
  </div>
  <button class="zoom-btn" onclick={zoomOut} disabled={cardSize <= MIN_SIZE} title="Smaller">-</button>
  <button class="zoom-btn" onclick={zoomIn} disabled={cardSize >= MAX_SIZE} title="Larger">+</button>
</div>

{#if items.length === 0}
  <div class="empty">
    <p>No media yet. Drag and drop files here to import.</p>
  </div>
{:else}
  <div class="grid" style="grid-template-columns: repeat(auto-fill, minmax({cardSize}px, 1fr));">
    {#each items as item (item.id)}
      <MediaCard media={item} score={scores?.[item.id]} {onMediaDeleted} {onEditDescription} />
    {/each}
  </div>
  {#if hasMore}
    <div class="load-more-sentinel" bind:this={sentinelEl}></div>
  {/if}
{/if}

<style>
  .toolbar {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .volume-control {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-right: auto;
  }

  .volume-icon {
    font-size: 1rem;
    width: 20px;
    text-align: center;
  }

  .volume-slider {
    width: 100px;
    height: 4px;
    cursor: pointer;
    accent-color: var(--accent);
  }

  .zoom-btn {
    width: 32px;
    height: 32px;
    padding: 0;
    font-size: 1.1rem;
    font-weight: bold;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: var(--bg-card);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .zoom-btn:hover:not(:disabled) {
    background-color: var(--border);
  }

  .zoom-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .grid {
    display: grid;
    gap: 12px;
  }

  .load-more-sentinel {
    height: 1px;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 300px;
    color: var(--text-muted);
    border: 2px dashed var(--border);
    border-radius: 12px;
    margin: 32px;
  }
</style>
