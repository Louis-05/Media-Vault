<script lang="ts">
  import { currentPage, editMediaId } from "../stores/vault";
  import {
    getMediaForDescription,
    getFilteredMediaIds,
    getMediaById,
    getMediaPath,
    getMediaIndex,
    toMediaUrl,
    revealMedia,
    copyMediaFile,
    copyMediaPath,
    deleteMedia,
    getMediaTags,
    setMediaTags,
    getAllTagKeys,
    getTagValues,
    createTagKey,
    getMissingCounts,
  } from "../api";
  import type { DescriptionPageData, TagInfo, TagKeyInfo, MediaInfo } from "../api";
  import { onMount } from "svelte";
  import { get } from "svelte/store";

  let data = $state<DescriptionPageData | null>(null);
  let descriptionText = $state("");
  let lastSavedText = $state("");
  let filterMissingDesc = $state(false);
  let filterMissingTags = $state(false);
  let missingDesc = $state(0);
  let missingTags = $state(0);
  let missingBoth = $state(0);
  let currentIndex = $state(0);
  let mediaSrc = $state<string | null>(null);
  let mediaType = $state<string>("");
  let saveTimeout: ReturnType<typeof setTimeout> | undefined;
  let textareaEl = $state<HTMLTextAreaElement | null>(null);

  // Cached filtered media IDs — only refreshed on filter change or Refresh button
  let cachedIds = $state<string[] | null>(null);
  let filtersActive = $derived(filterMissingDesc || filterMissingTags);

  function globalKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && document.activeElement === textareaEl) {
      e.preventDefault();
      handleNext();
    }
  }

  onMount(async () => {
    window.addEventListener("keydown", globalKeydown);

    const targetId = get(editMediaId);
    if (targetId) {
      editMediaId.set(null);
      filterMissingDesc = false;
      filterMissingTags = false;
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
      const idx = await getMediaIndex(mediaId, false, false);
      if (idx !== null) {
        currentIndex = idx;
      }
    } catch (e) {
      console.error("Failed to find media index:", e);
    }
    await loadCurrent();
  }

  async function refreshMissingCounts() {
    try {
      const [d, t, b] = await getMissingCounts();
      missingDesc = d;
      missingTags = t;
      missingBoth = b;
    } catch (e) {
      console.error("Failed to load missing counts:", e);
    }
  }

  async function loadMediaInfo(media: MediaInfo, index: number, totalCount: number) {
    descriptionText = media.description ?? "";
    lastSavedText = descriptionText;
    currentIndex = index;
    mediaType = media.media_type;
    data = {
      media,
      description: media.description ?? null,
      current_index: index,
      total_count: totalCount,
    };
    try {
      const path = await getMediaPath(media.id);
      mediaSrc = toMediaUrl(path);
    } catch {
      mediaSrc = null;
    }
    await loadTags();
    refreshMissingCounts();
    requestAnimationFrame(() => textareaEl?.focus());
  }

  async function loadCurrent() {
    try {
      if (cachedIds && filtersActive) {
        // Navigate within the cached filtered set
        if (cachedIds.length === 0) {
          data = null;
          descriptionText = "";
          mediaSrc = null;
          mediaType = "";
          return;
        }
        const clampedIndex = Math.min(currentIndex, cachedIds.length - 1);
        const mediaId = cachedIds[clampedIndex];
        const media = await getMediaById(mediaId);
        if (media) {
          await loadMediaInfo(media, clampedIndex, cachedIds.length);
        } else {
          data = null;
          descriptionText = "";
          mediaSrc = null;
          mediaType = "";
        }
      } else {
        // No filters — use index-based query
        const result = await getMediaForDescription(currentIndex, false, false);
        if (result) {
          await loadMediaInfo(result.media, result.current_index, result.total_count);
        } else {
          data = null;
          descriptionText = "";
          mediaSrc = null;
          mediaType = "";
        }
      }
    } catch (e) {
      console.error("Failed to load description data:", e);
    }
  }

  async function saveNow() {
    if (!data || descriptionText === lastSavedText) return;
    try {
      // Update the description tag within mediaTags (same path as all other tags)
      const descTag = mediaTags.find((t) => t.key === "description");
      if (descTag) {
        descTag.value = descriptionText;
      } else {
        mediaTags = [...mediaTags, { key: "description", value: descriptionText }];
      }
      await setMediaTags(data.media.id, mediaTags);
      lastSavedText = descriptionText;
      data.media.has_description = true;
      data.description = descriptionText;
      refreshMissingCounts();
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
    if (currentIndex < data.total_count - 1) {
      navigateTo(currentIndex + 1);
    }
  }

  async function refreshFilteredSet() {
    if (filterMissingDesc || filterMissingTags) {
      cachedIds = await getFilteredMediaIds(filterMissingDesc, filterMissingTags);
    } else {
      cachedIds = null;
    }
    currentIndex = 0;
    await loadCurrent();
  }

  async function handleRefreshFilter() {
    if (saveTimeout) clearTimeout(saveTimeout);
    await saveNow();
    await refreshFilteredSet();
  }

  async function handleFilterDescChange(e: Event) {
    filterMissingDesc = (e.target as HTMLInputElement).checked;
    await refreshFilteredSet();
  }

  async function handleFilterTagsChange(e: Event) {
    filterMissingTags = (e.target as HTMLInputElement).checked;
    await refreshFilteredSet();
  }

  // --- Tags ---
  let mediaTags = $state<TagInfo[]>([]);
  let allTagKeys = $state<TagKeyInfo[]>([]);
  let newTagKey = $state("");
  let newTagValue = $state("");
  let tagKeySuggestions = $state<string[]>([]);
  let showKeySuggestions = $state(false);
  let editingTagIndex = $state<number | null>(null);
  let editingTagValue = $state("");
  let knownValues = $state<string[]>([]);
  let tagValueSuggestions = $state<string[]>([]);
  let showValueSuggestions = $state(false);

  function levenshtein(a: string, b: string): number {
    const m = a.length, n = b.length;
    const d: number[][] = Array.from({ length: m + 1 }, (_, i) =>
      Array.from({ length: n + 1 }, (_, j) => (i === 0 ? j : j === 0 ? i : 0))
    );
    for (let i = 1; i <= m; i++)
      for (let j = 1; j <= n; j++)
        d[i][j] = Math.min(
          d[i - 1][j] + 1,
          d[i][j - 1] + 1,
          d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1)
        );
    return d[m][n];
  }

  function updateTagKeySuggestions() {
    const input = newTagKey.toLowerCase().trim();
    if (!input) {
      tagKeySuggestions = [];
      showKeySuggestions = false;
      return;
    }
    const keys = allTagKeys.map((k) => k.key).filter((k) => k !== "media_type");
    const scored = keys.map((k) => {
      const kl = k.toLowerCase();
      if (kl === input) return { key: k, score: 0 };
      if (kl.startsWith(input)) return { key: k, score: 1 };
      if (kl.includes(input)) return { key: k, score: 2 };
      const dist = levenshtein(kl, input);
      if (dist <= 2) return { key: k, score: 3 + dist };
      return null;
    }).filter(Boolean) as { key: string; score: number }[];
    scored.sort((a, b) => a.score - b.score);
    tagKeySuggestions = scored.map((s) => s.key).slice(0, 8);
    showKeySuggestions = true;
  }

  function selectTagKey(key: string) {
    newTagKey = key;
    showKeySuggestions = false;
  }

  // Reload known values whenever the selected tag key changes
  let lastLoadedKey = "";
  $effect(() => {
    const key = newTagKey.trim();
    if (key !== lastLoadedKey) {
      lastLoadedKey = key;
      knownValues = [];
      tagValueSuggestions = [];
      showValueSuggestions = false;
      if (key) {
        getTagValues(key).then((values) => {
          // Only apply if key hasn't changed since we started loading
          if (newTagKey.trim() === key) {
            knownValues = values;
          }
        }).catch(() => {
          knownValues = [];
        });
      }
    }
  });

  function updateValueSuggestions() {
    const input = newTagValue.toLowerCase().trim();
    if (!input || knownValues.length === 0) {
      tagValueSuggestions = [];
      showValueSuggestions = false;
      return;
    }
    const scored = knownValues
      .filter((v) => v.toLowerCase().includes(input) || levenshtein(v.toLowerCase(), input) <= 2)
      .filter((v) => v.toLowerCase() !== input);
    tagValueSuggestions = scored.slice(0, 8);
    showValueSuggestions = tagValueSuggestions.length > 0;
  }

  function selectTagValue(value: string) {
    newTagValue = value;
    showValueSuggestions = false;
  }

  async function loadTags() {
    if (!data) return;
    try {
      mediaTags = await getMediaTags(data.media.id);
      allTagKeys = await getAllTagKeys();
    } catch (e) {
      console.error("Failed to load tags:", e);
    }
  }

  async function addTag() {
    if (!data || !newTagKey.trim() || !newTagValue.trim()) return;
    const key = newTagKey.trim();
    const value = newTagValue.trim();

    // Ensure key exists
    if (!allTagKeys.some((k) => k.key === key)) {
      await createTagKey(key);
    }

    // Add to current tags (avoid duplicates)
    const exists = mediaTags.some((t) => t.key === key && t.value === value);
    if (!exists) {
      const updated = [...mediaTags, { key, value }];
      await setMediaTags(data.media.id, updated);
      mediaTags = updated;
    }

    newTagValue = "";
    allTagKeys = await getAllTagKeys();
  }

  async function removeTag(index: number) {
    if (!data) return;
    const updated = mediaTags.filter((_, i) => i !== index);
    await setMediaTags(data.media.id, updated);
    mediaTags = updated;
    editingTagIndex = null;
  }

  function startEditTag(tagIndex: number) {
    editingTagIndex = tagIndex;
    editingTagValue = mediaTags[tagIndex].value;
  }

  async function finishEditTag() {
    if (editingTagIndex === null || !data) return;
    const newVal = editingTagValue.trim();
    if (!newVal || newVal === mediaTags[editingTagIndex].value) {
      editingTagIndex = null;
      return;
    }
    const updated = mediaTags.map((t, i) =>
      i === editingTagIndex ? { ...t, value: newVal } : t
    );
    await setMediaTags(data.media.id, updated);
    mediaTags = updated;
    editingTagIndex = null;
  }

  // Non-system tags (exclude media_type which is auto)
  let editableTags = $derived(mediaTags.filter((t) => t.key !== "media_type" && t.key !== "description"));

  // Suggested tag keys: commonly used keys not yet on this media
  let suggestedKeys = $derived(() => {
    const usedKeys = new Set(mediaTags.map((t) => t.key));
    return allTagKeys
      .filter((k) => !usedKeys.has(k.key) && k.key !== "media_type" && k.key !== "description" && k.usage_count > 0)
      .sort((a, b) => b.usage_count - a.usage_count)
      .slice(0, 5)
      .map((k) => k.key);
  });

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
      const deletedId = data.media.id;
      await deleteMedia(deletedId);
      confirmDelete = false;
      // Remove from cached filtered set if active
      if (cachedIds) {
        cachedIds = cachedIds.filter((id) => id !== deletedId);
      }
      // If we were at the end, step back so we don't overshoot
      if (cachedIds && filtersActive) {
        if (currentIndex >= cachedIds.length && currentIndex > 0) {
          currentIndex = currentIndex - 1;
        }
      }
      // Reset saved state so saveNow() doesn't try to save to deleted media
      lastSavedText = descriptionText;
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
          <input
            class="counter-input"
            type="number"
            min="1"
            max={data.total_count}
            value={currentIndex + 1}
            onkeydown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                e.stopPropagation();
                const val = parseInt((e.target as HTMLInputElement).value);
                if (val >= 1 && val <= (data?.total_count ?? 0)) {
                  navigateTo(val - 1);
                }
              }
            }}
            onblur={(e) => {
              const val = parseInt((e.target as HTMLInputElement).value);
              if (val >= 1 && val <= (data?.total_count ?? 0) && val - 1 !== currentIndex) {
                navigateTo(val - 1);
              }
            }}
          /> / {data.total_count}
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
        checked={filterMissingDesc}
        onchange={handleFilterDescChange}
      />
      Missing descriptions
    </label>
    <label class="filter">
      <input
        type="checkbox"
        checked={filterMissingTags}
        onchange={handleFilterTagsChange}
      />
      Missing tags
    </label>
    {#if filterMissingDesc || filterMissingTags}
      <button class="secondary refresh-btn" onclick={handleRefreshFilter} title="Refresh filtered list">↻ Refresh</button>
    {/if}
    <div class="missing-stats">
      <span>{missingDesc} no desc</span>
      <span>{missingTags} no tags</span>
      <span>{missingBoth} no desc+tags</span>
    </div>
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
        <!-- Tags Section -->
        <div class="tags-section">
          <h3>Tags</h3>
          {#if editableTags.length > 0}
            <div class="tag-list">
              {#each editableTags as tag, i}
                {@const realIndex = mediaTags.indexOf(tag)}
                <div class="tag-chip">
                  <span class="tag-key">{tag.key}</span>
                  {#if editingTagIndex === realIndex}
                    <input
                      class="tag-edit-input"
                      type="text"
                      bind:value={editingTagValue}
                      onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); finishEditTag(); } if (e.key === "Escape") editingTagIndex = null; }}
                      onblur={finishEditTag}
                      autofocus
                    />
                  {:else}
                    <span class="tag-value" onclick={() => startEditTag(realIndex)} title="Click to edit">{tag.value}</span>
                  {/if}
                  <button class="tag-remove" onclick={() => removeTag(realIndex)}>×</button>
                </div>
              {/each}
            </div>
          {/if}

          <div class="tag-add">
            <div class="tag-key-input-wrapper">
              <input
                type="text"
                placeholder="Tag key..."
                bind:value={newTagKey}
                oninput={updateTagKeySuggestions}
                onfocus={updateTagKeySuggestions}
                onblur={() => setTimeout(() => showKeySuggestions = false, 200)}
                onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); } }}
              />
              {#if showKeySuggestions && tagKeySuggestions.length > 0}
                <div class="suggestions">
                  {#each tagKeySuggestions as suggestion}
                    <button class="suggestion" onmousedown={() => selectTagKey(suggestion)}>
                      {suggestion}
                    </button>
                  {/each}
                  {#if newTagKey.trim() && !tagKeySuggestions.includes(newTagKey.trim())}
                    <button class="suggestion create-new" onmousedown={() => selectTagKey(newTagKey.trim())}>
                      + Create "{newTagKey.trim()}"
                    </button>
                  {/if}
                </div>
              {:else if showKeySuggestions && newTagKey.trim()}
                <div class="suggestions">
                  <button class="suggestion create-new" onmousedown={() => selectTagKey(newTagKey.trim())}>
                    + Create "{newTagKey.trim()}"
                  </button>
                </div>
              {/if}
            </div>
            <div class="tag-value-input-wrapper">
              <input
                type="text"
                placeholder="Value..."
                bind:value={newTagValue}
                oninput={updateValueSuggestions}
                onfocus={updateValueSuggestions}
                onblur={() => setTimeout(() => showValueSuggestions = false, 200)}
                onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); addTag(); } }}
              />
              {#if showValueSuggestions && tagValueSuggestions.length > 0}
                <div class="suggestions">
                  {#each tagValueSuggestions as suggestion}
                    <button class="suggestion" onmousedown={() => selectTagValue(suggestion)}>
                      {suggestion}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
            <button class="secondary" onclick={addTag} disabled={!newTagKey.trim() || !newTagValue.trim()}>Add</button>
          </div>

          {#if suggestedKeys().length > 0}
            <div class="tag-suggestions">
              <span class="suggestions-label">Suggested:</span>
              {#each suggestedKeys() as key}
                <button class="suggestion-chip" onclick={() => { newTagKey = key; showKeySuggestions = false; }}>
                  {key}
                </button>
              {/each}
            </div>
          {/if}
        </div>

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
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .counter-input {
    width: 50px;
    padding: 2px 4px;
    font-size: 0.9rem;
    text-align: center;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    -moz-appearance: textfield;
  }

  .counter-input::-webkit-outer-spin-button,
  .counter-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .missing-stats {
    display: flex;
    gap: 12px;
    margin-left: auto;
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
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

  .refresh-btn {
    font-size: 0.8rem;
    padding: 4px 10px;
  }

  main {
    flex: 1;
    display: flex;
    flex-direction: row;
    padding: 24px;
    gap: 24px;
    overflow-y: auto;
  }

  .preview-container {
    flex-shrink: 0;
    width: 45%;
    max-width: 500px;
  }

  .preview {
    width: 100%;
    aspect-ratio: 1;
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
    flex: 1;
    min-width: 0;
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

  /* Tags */
  .tags-section {
    width: 100%;
  }

  .tags-section h3 {
    margin: 0 0 8px 0;
    font-size: 0.85rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }

  .tag-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    background-color: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 3px 6px;
    font-size: 0.8rem;
  }

  .tag-key {
    color: var(--accent);
    font-weight: 600;
  }

  .tag-value {
    color: var(--text);
    cursor: pointer;
  }

  .tag-value:hover {
    text-decoration: underline;
    text-decoration-style: dashed;
  }

  .tag-edit-input {
    width: 80px;
    padding: 0 4px;
    font-size: 0.8rem;
    border: 1px solid var(--accent);
    border-radius: 2px;
    background: var(--bg);
    color: var(--text);
  }

  .tag-remove {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.9rem;
    padding: 0 2px;
    line-height: 1;
  }

  .tag-remove:hover {
    color: #ff4444;
  }

  .tag-add {
    display: flex;
    gap: 6px;
    align-items: flex-start;
  }

  .tag-key-input-wrapper {
    position: relative;
    flex: 1;
  }

  .tag-value-input-wrapper {
    position: relative;
    flex: 1;
  }

  .tag-key-input-wrapper input,
  .tag-value-input-wrapper input,
  .tag-add > input {
    width: 100%;
    padding: 6px 10px;
    font-size: 0.85rem;
    border-radius: 4px;
  }

  .tag-add > input {
    flex: 1;
  }

  .tag-add button {
    padding: 6px 12px;
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .suggestions {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    box-shadow: 0 4px 12px var(--shadow);
    z-index: 100;
    max-height: 200px;
    overflow-y: auto;
  }

  .suggestion {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    background: none;
    border: none;
    border-radius: 0;
    color: var(--text);
    font-size: 0.85rem;
    cursor: pointer;
  }

  .suggestion:hover {
    background: var(--border);
  }

  .suggestion.create-new {
    color: var(--accent);
    font-style: italic;
  }

  .tag-suggestions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    flex-wrap: wrap;
  }

  .suggestions-label {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .suggestion-chip {
    padding: 2px 8px;
    font-size: 0.75rem;
    border: 1px dashed var(--border);
    border-radius: 4px;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }

  .suggestion-chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
