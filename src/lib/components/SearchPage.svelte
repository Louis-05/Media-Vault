<script lang="ts">
  import { currentPage, currentVault, mediaList, editMediaId, isDraggingOut } from "../stores/vault";
  import { get } from "svelte/store";
  import { getMediaList, searchMedia, importMedia, openVault, createVault, refreshVault } from "../api";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import MediaGrid from "./MediaGrid.svelte";
  import SearchBar from "./SearchBar.svelte";

  let searchQuery = $state("");
  let searchResults = $state<any[]>([]);
  let isSearching = $state(false);
  let searchTimeout: ReturnType<typeof setTimeout> | undefined;
  let notification = $state<string | null>(null);
  let notificationTimeout: ReturnType<typeof setTimeout> | undefined;
  let hasMore = $state(true);
  let loadingMore = $state(false);
  const PAGE_SIZE = 200;

  onMount(() => {
    loadMedia();

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

    // Refresh grid when media changes (imports, deletes, refresh)
    const unlistenChanged = listen("media-changed", async () => {
      await loadMedia();
      if (searchQuery.trim()) {
        try {
          const results = await searchMedia(searchQuery, 50);
          searchResults = results;
        } catch (e) {
          console.error("Search re-run failed:", e);
        }
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

    return () => {
      unlistenDrop.then((fn) => fn());
      unlistenChanged.then((fn) => fn());
      unlistenDuplicates.then((fn) => fn());
    };
  });

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

  function handleSearch(query: string) {
    searchQuery = query;
    if (searchTimeout) clearTimeout(searchTimeout);

    if (!query.trim()) {
      searchResults = [];
      isSearching = false;
      return;
    }

    isSearching = true;
    searchTimeout = setTimeout(async () => {
      try {
        const results = await searchMedia(query, 50);
        searchResults = results;
      } catch (e) {
        console.error("Search failed:", e);
      }
      isSearching = false;
    }, 300);
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
    } catch (e) {
      console.error("Failed to open vault:", e);
    }
  }

  async function handleRefresh() {
    try {
      await refreshVault();
      await loadMedia();
      if (searchQuery.trim()) {
        const results = await searchMedia(searchQuery, 50);
        searchResults = results;
      }
    } catch (e) {
      console.error("Refresh failed:", e);
    }
  }

  function goToDescriptions() {
    editMediaId.set(null);
    currentPage.set("descriptions");
  }

  function handleEditDescription(mediaId: string) {
    editMediaId.set(mediaId);
    currentPage.set("descriptions");
  }

  $effect(() => {
    // Reload media when vault changes
    if ($currentVault) {
      loadMedia();
    }
  });
</script>

<div class="search-page">
  <header>
    <div class="header-left">
      <div class="menu">
        <button class="secondary" onclick={handleOpenFolder}>Open Folder</button>
        <button class="secondary" onclick={handleRefresh} title="Refresh">Refresh</button>
        <button class="secondary" onclick={goToDescriptions}>Descriptions</button>
      </div>
    </div>
    <div class="header-center">
      <SearchBar onSearch={handleSearch} />
    </div>
    <div class="header-right"></div>
  </header>

  {#if notification}
    <div class="notification">{notification}</div>
  {/if}

  <main>
    {#if searchQuery.trim() && searchResults.length > 0}
      <MediaGrid items={searchResults.map((r) => r.media)} scores={Object.fromEntries(searchResults.map((r) => [r.media.id, r.score]))} onMediaDeleted={loadMedia} onEditDescription={handleEditDescription} />
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
    width: 200px;
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
