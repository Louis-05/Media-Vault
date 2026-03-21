<script lang="ts">
  import { currentPage, editMediaId } from "../stores/vault";
  import {
    getMediaForDescription,
    setDescription,
    getMediaPath,
    getMediaIndex,
    toMediaUrl,
    revealMedia,
    copyMediaFile,
    copyMediaPath,
    deleteMedia,
  } from "../api";
  import type { DescriptionPageData } from "../api";
  import { onMount } from "svelte";
  import { get } from "svelte/store";

  let data = $state<DescriptionPageData | null>(null);
  let descriptionText = $state("");
  let lastSavedText = $state("");
  let filterMissing = $state(true);
  let currentIndex = $state(0);
  let mediaSrc = $state<string | null>(null);
  let mediaType = $state<string>("");
  let saveTimeout: ReturnType<typeof setTimeout> | undefined;
  let textareaEl = $state<HTMLTextAreaElement | null>(null);

  function globalKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleNext();
    }
  }

  onMount(async () => {
    window.addEventListener("keydown", globalKeydown);

    const targetId = get(editMediaId);
    if (targetId) {
      editMediaId.set(null);
      filterMissing = false;
      await loadSpecificMedia(targetId);
    } else {
      await loadCurrent();
    }

    return () => {
      window.removeEventListener("keydown", globalKeydown);
    };
  });

  async function loadSpecificMedia(mediaId: string) {
    try {
      const idx = await getMediaIndex(mediaId, filterMissing);
      if (idx !== null) {
        currentIndex = idx;
      }
    } catch (e) {
      console.error("Failed to find media index:", e);
    }
    await loadCurrent();
  }

  async function loadCurrent() {
    try {
      const result = await getMediaForDescription(currentIndex, filterMissing);
      data = result;
      if (result) {
        descriptionText = result.description ?? "";
        lastSavedText = descriptionText;
        currentIndex = result.current_index;
        mediaType = result.media.media_type;
        try {
          const path = await getMediaPath(result.media.id);
          mediaSrc = toMediaUrl(path);
        } catch {
          mediaSrc = null;
        }
        // Focus textarea after render
        requestAnimationFrame(() => textareaEl?.focus());
      } else {
        descriptionText = "";
        mediaSrc = null;
        mediaType = "";
      }
    } catch (e) {
      console.error("Failed to load description data:", e);
    }
  }

  async function saveNow() {
    if (!data || descriptionText === lastSavedText) return;
    try {
      await setDescription(data.media.id, descriptionText);
      lastSavedText = descriptionText;
      data.media.has_description = true;
      data.description = descriptionText;
    } catch (e) {
      console.error("Failed to save:", e);
    }
  }

  function scheduleAutoSave() {
    if (saveTimeout) clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => saveNow(), 1000);
  }

  async function navigateTo(index: number) {
    if (saveTimeout) clearTimeout(saveTimeout);
    await saveNow();
    currentIndex = index;
    await loadCurrent();
  }

  function handlePrev() {
    if (currentIndex > 0) {
      navigateTo(currentIndex - 1);
    }
  }

  function handleNext() {
    if (!data) return;
    if (filterMissing && descriptionText.trim() && descriptionText !== (data.description ?? "")) {
      // Current media will leave the "missing" set after save — stay at same index
      navigateTo(currentIndex);
    } else if (currentIndex < data.total_count - 1) {
      navigateTo(currentIndex + 1);
    }
  }


  function handleFilterChange(e: Event) {
    filterMissing = (e.target as HTMLInputElement).checked;
    currentIndex = 0;
    loadCurrent();
  }

  let confirmDelete = $state(false);

  async function handleReveal() {
    if (!data) return;
    try { await revealMedia(data.media.id); } catch (e) { console.error("Reveal failed:", e); }
  }

  async function handleCopyFile() {
    if (!data) return;
    try { await copyMediaFile(data.media.id); } catch (e) { console.error("Copy failed:", e); }
  }

  async function handleCopyPath() {
    if (!data) return;
    try { await copyMediaPath(data.media.id); } catch (e) { console.error("Copy path failed:", e); }
  }

  async function handleDelete() {
    if (!data) return;
    try {
      await deleteMedia(data.media.id);
      confirmDelete = false;
      await loadCurrent();
    } catch (e) { console.error("Delete failed:", e); }
  }

  async function goBack() {
    if (saveTimeout) clearTimeout(saveTimeout);
    await saveNow();
    currentPage.set("search");
  }
</script>

<div class="descriptions-page">
  <header>
    <button class="secondary" onclick={goBack}>← Back to Search</button>
    <div class="nav">
      <button class="secondary" onclick={handlePrev} disabled={currentIndex === 0}>
        ← Prev
      </button>
      <span class="counter">
        {#if data}
          {currentIndex + 1} / {data.total_count}
        {:else}
          0 / 0
        {/if}
      </span>
      <button
        class="secondary"
        onclick={handleNext}
        disabled={!data || currentIndex >= data.total_count - 1}
      >
        Next →
      </button>
    </div>
    <label class="filter">
      <input
        type="checkbox"
        checked={filterMissing}
        onchange={handleFilterChange}
      />
      Only missing descriptions
    </label>
  </header>

  {#if data}
    <main>
      <div class="preview-container">
        <div class="preview">
          {#if mediaSrc && mediaType === "video"}
            <video controls autoplay loop src={mediaSrc}>
              <track kind="captions" />
            </video>
          {:else if mediaSrc && mediaType === "audio"}
            <!-- svelte-ignore a11y_media_has_caption -->
            <audio controls autoplay loop src={mediaSrc}></audio>
          {:else if mediaSrc}
            <img src={mediaSrc} alt={data.media.id} />
          {:else}
            <div class="placeholder">No preview</div>
          {/if}
        </div>
        <p class="filename">{data.media.id}.{data.media.extension}</p>
      </div>
      <div class="editor">
        <textarea
          bind:this={textareaEl}
          bind:value={descriptionText}
          placeholder="Describe this media..."
          rows="6"
          oninput={scheduleAutoSave}
        ></textarea>
        <div class="actions">
          <button class="secondary action-btn" onclick={handleReveal}>Reveal in explorer</button>
          <button class="secondary action-btn" onclick={handleCopyFile}>Copy media</button>
          <button class="secondary action-btn" onclick={handleCopyPath}>Copy path</button>
          {#if confirmDelete}
            <div class="delete-confirm">
              <span>Are you sure?</span>
              <button class="delete-yes" onclick={handleDelete}>Delete</button>
              <button class="delete-no" onclick={() => confirmDelete = false}>Cancel</button>
            </div>
          {:else}
            <button class="secondary action-btn delete-btn" onclick={() => confirmDelete = true}>Delete</button>
          {/if}
        </div>
      </div>
    </main>
  {:else}
    <div class="empty">
      <p>No media to describe.</p>
    </div>
  {/if}
</div>

<style>
  .descriptions-page {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  header {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    background-color: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    gap: 16px;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-left: auto;
  }

  .counter {
    font-size: 0.9rem;
    color: var(--text-muted);
    min-width: 60px;
    text-align: center;
  }

  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
    color: var(--text-muted);
    cursor: pointer;
    margin-left: 16px;
  }

  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32px;
    gap: 24px;
    overflow-y: auto;
  }

  .preview-container {
    text-align: center;
  }

  .preview {
    width: 500px;
    height: 400px;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: var(--bg-surface);
    border-radius: 8px;
    overflow: hidden;
  }

  .preview img,
  .preview video {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .filename {
    margin-top: 8px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .placeholder {
    color: var(--text-muted);
  }

  .editor {
    width: 100%;
    max-width: 600px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  textarea {
    resize: vertical;
    min-height: 100px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .action-btn {
    font-size: 0.8rem;
    padding: 6px 12px;
  }

  .delete-btn {
    color: #ff4444 !important;
    border-color: #ff4444 !important;
    margin-left: auto;
  }

  .delete-btn:hover {
    background-color: rgba(255, 68, 68, 0.15) !important;
  }

  .delete-confirm {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .delete-yes {
    background-color: #ff4444;
    color: white;
    border: none;
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .delete-yes:hover {
    background-color: #cc3333;
  }

  .delete-no {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 0.8rem;
    background: none;
    color: var(--text);
    cursor: pointer;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 300px;
    color: var(--text-muted);
  }
</style>
