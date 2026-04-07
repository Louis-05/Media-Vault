<script lang="ts">
  import { currentPage, currentVault, mediaList, editMediaId, isDraggingOut, loadingStatus, processedCount, autoSearch, searchQueryStore, searchResultsStore, searchHasMoreStore, mediaHasMoreStore, searchScrollTop } from "../stores/vault";
  import { get } from "svelte/store";
  import { getMediaList, searchMedia, importMedia, openVault, createVault, getProcessedCount, pauseProcessing, resumeProcessing } from "../api";
  import type { MediaInfo, SearchResult } from "../api";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import MediaGrid from "./MediaGrid.svelte";
  import SearchBar from "./SearchBar.svelte";

  let searchQuery = $state(get(searchQueryStore));
  let searchResults = $state<SearchResult[]>(get(searchResultsStore));
  let isSearching = $state(false);
  let notification = $state<string | null>(null);
  let notificationTimeout: ReturnType<typeof setTimeout> | undefined;
  let hasMore = $state(get(mediaHasMoreStore));
  let searchHasMore = $state(get(searchHasMoreStore));
  let loadingMore = $state(false);
  let paused = $state(false);
  let mainEl = $state<HTMLElement | null>(null);
  const PAGE_SIZE = 200;

  // Mirror local state into the persistence stores so navigation away preserves them.
  $effect(() => { searchQueryStore.set(searchQuery); });
  $effect(() => { searchResultsStore.set(searchResults); });
  $effect(() => { searchHasMoreStore.set(searchHasMore); });
  $effect(() => { mediaHasMoreStore.set(hasMore); });

  onMount(() => {
    // Only fetch the media list if we don't already have one cached in the store
    // (e.g. on first mount). Returning from another view should keep the existing list.
    if (get(mediaList).length === 0) {
      loadMedia();
    }
    loadProcessedCount();

    // Restore scroll position after the grid has rendered
    tick().then(() => {
      if (mainEl) mainEl.scrollTop = get(searchScrollTop);
    });

    // Listen for drag-and-drop file imports
    const unlistenDrop = listen<{ paths: string[] }>("tauri://drag-drop", async (event) => {
      if (get(isDraggingOut)) return;
      if (event.payload.paths && event.payload.paths.length > 0) {
        const vault = get(currentVault);
        const mediaDir = vault ? vault.path + "/media/" : null;
        const externalPaths = mediaDir
          ? event.payload.paths.filter((p) => !p.startsWith(mediaDir))
          : event.payload.paths;
        if (externalPaths.length > 0) {
          await importMedia(externalPaths);
        }
      }
    });

    // Live-add newly processed media to the grid
    const unlistenProcessed = listen<MediaInfo>("media-processed", async (event) => {
      const media = event.payload;

      // Always fetch real count from backend to avoid drift
      await loadProcessedCount();

      if (!searchQuery.trim()) {
        // Append to end of grid only if not already present
        mediaList.update((list) => {
          if (list.some((m) => m.id === media.id)) return list;
          return [...list, media];
        });
      }
    });

    const unlistenDuplicates = listen<number>("duplicates-skipped", (event) => {
      const count = event.payload;
      notification = count === 1
        ? "1 duplicate file was ignored"
        : `${count} duplicate files were ignored`;
      if (notificationTimeout) clearTimeout(notificationTimeout);
      notificationTimeout = setTimeout(() => { notification = null; }, 4000);
    });

    // Listen for media-changed (from deep refresh cleanup)
    const unlistenChanged = listen("media-changed", async () => {
      await loadMedia();
      await loadProcessedCount();
    });

    return () => {
      unlistenDrop.then((fn) => fn());
      unlistenProcessed.then((fn) => fn());
      unlistenDuplicates.then((fn) => fn());
      unlistenChanged.then((fn) => fn());
    };
  });

  async function loadProcessedCount() {
    try {
      const count = await getProcessedCount();
      processedCount.set(count);
    } catch (e) {
      console.error("Failed to load processed count:", e);
    }
  }

  async function loadMedia() {
    try {
      const list = await getMediaList(0, PAGE_SIZE);
      mediaList.set(list);
      hasMore = list.length >= PAGE_SIZE;
    } catch (e) {
      console.error("Failed to load media:", e);
    }
  }

  async function loadMore() {
    if (loadingMore || !hasMore) return;
    loadingMore = true;
    try {
      const current = get(mediaList);
      const list = await getMediaList(current.length, PAGE_SIZE);
      mediaList.set([...current, ...list]);
      hasMore = list.length >= PAGE_SIZE;
    } catch (e) {
      console.error("Failed to load more media:", e);
    }
    loadingMore = false;
  }

  async function handleSearch(query: string) {
    searchQuery = query;

    if (!query.trim()) {
      searchResults = [];
      isSearching = false;
      searchHasMore = true;
      return;
    }

    isSearching = true;
    try {
      const results = await searchMedia(query, 0, PAGE_SIZE);
      searchResults = results;
      searchHasMore = results.length >= PAGE_SIZE;
    } catch (e) {
      console.error("Search failed:", e);
    }
    isSearching = false;
  }

  async function loadMoreSearch() {
    if (loadingMore || !searchHasMore || !searchQuery.trim()) return;
    loadingMore = true;
    try {
      const results = await searchMedia(searchQuery, searchResults.length, PAGE_SIZE);
      searchResults = [...searchResults, ...results];
      searchHasMore = results.length >= PAGE_SIZE;
    } catch (e) {
      console.error("Search load more failed:", e);
    }
    loadingMore = false;
  }

  async function handleOpenFolder() {
    const selected = await open({ directory: true });
    if (!selected) return;
    try {
      try {
        const info = await openVault(selected);
        currentVault.set(info);
      } catch {
        const info = await createVault(selected);
        currentVault.set(info);
      }
      await loadMedia();
      await loadProcessedCount();
    } catch (e) {
      console.error("Failed to open vault:", e);
    }
  }

  function goToDescriptions() {
    editMediaId.set(null);
    currentPage.set("descriptions");
  }

  function goToSettings() {
    currentPage.set("settings");
  }

  function handleEditDescription(mediaId: string) {
    editMediaId.set(mediaId);
    currentPage.set("descriptions");
  }

  async function togglePause() {
    if (paused) {
      await resumeProcessing();
      paused = false;
    } else {
      await pauseProcessing();
      paused = true;
    }
  }

  // Reload media only when the vault actually changes after this component is mounted
  // (not on every mount — that would clobber cached results when returning from another view).
  let lastVaultPath = $state<string | null>(get(currentVault)?.path ?? null);
  $effect(() => {
    const v = $currentVault;
    if (v && v.path !== lastVaultPath) {
      lastVaultPath = v.path;
      searchQuery = "";
      searchResults = [];
      searchHasMore = true;
      loadMedia();
      loadProcessedCount();
      searchScrollTop.set(0);
      if (mainEl) mainEl.scrollTop = 0;
    }
  });
</script>

<div class="search-page">
  <header>
    <div class="header-left">
      <div class="menu">
        <button class="secondary" onclick={handleOpenFolder}>Open Folder</button>
        <button class="secondary" onclick={goToDescriptions}>Descriptions</button>
        <button class="secondary" onclick={() => currentPage.set("tags")}>Tags</button>
        <button class="secondary" onclick={goToSettings}>Settings</button>
      </div>
    </div>
    <div class="header-center">
      <SearchBar onSearch={handleSearch} auto={$autoSearch} initialQuery={searchQuery} />
    </div>
    <div class="header-right">
      <span class="media-count">{$processedCount} media</span>
    </div>
  </header>

  {#if $loadingStatus}
    <div class="progress-bar">
      <div class="progress-spinner"></div>
      <span>{$loadingStatus}</span>
      <button class="pause-btn" onclick={togglePause}>
        {paused ? "Resume" : "Pause"}
      </button>
    </div>
  {/if}

  {#if notification}
    <div class="notification">{notification}</div>
  {/if}

  <main bind:this={mainEl} onscroll={() => { if (mainEl) searchScrollTop.set(mainEl.scrollTop); }}>
    {#if searchQuery.trim() && searchResults.length > 0}
      <MediaGrid items={searchResults.map((r) => r.media)} scores={Object.fromEntries(searchResults.map((r) => [r.media.id, r.score]))} onMediaDeleted={() => { handleSearch(searchQuery); loadMedia(); loadProcessedCount(); }} onEditDescription={handleEditDescription} onLoadMore={loadMoreSearch} hasMore={searchHasMore} />
    {:else if searchQuery.trim() && !isSearching}
      <div class="empty">No results found</div>
    {:else}
      <MediaGrid items={$mediaList} onMediaDeleted={loadMedia} onEditDescription={handleEditDescription} onLoadMore={loadMore} {hasMore} />
    {/if}
  </main>
</div>

<style>
  .search-page {
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

  .header-left {
    flex-shrink: 0;
  }

  .header-center {
    flex: 1;
    max-width: 600px;
    margin: 0 auto;
  }

  .header-right {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    min-width: 100px;
  }

  .media-count {
    color: var(--text-muted);
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .menu {
    display: flex;
    gap: 8px;
  }

  main {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-muted);
  }

  .progress-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px;
    background-color: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .progress-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  .pause-btn {
    margin-left: auto;
    padding: 2px 10px;
    font-size: 0.8rem;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-muted);
    border-radius: 4px;
    cursor: pointer;
  }

  .pause-btn:hover {
    background: var(--bg-surface);
    color: var(--text);
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .notification {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    background-color: var(--bg-surface);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 10px 20px;
    border-radius: 8px;
    font-size: 0.9rem;
    box-shadow: 0 4px 16px var(--shadow);
    z-index: 1000;
    animation: fade-in 0.2s ease;
  }

  @keyframes fade-in {
    from { opacity: 0; transform: translateX(-50%) translateY(10px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }
</style>
