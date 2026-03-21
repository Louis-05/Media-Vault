<script lang="ts">
  import { currentPage, currentVault, loadingStatus } from "./lib/stores/vault";
  import { getLoadingStatus, getLastVault, openVault } from "./lib/api";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import SearchPage from "./lib/components/SearchPage.svelte";
  import DescriptionsPage from "./lib/components/DescriptionsPage.svelte";
  import FolderPicker from "./lib/components/FolderPicker.svelte";
  import LoadingScreen from "./lib/components/LoadingScreen.svelte";

  onMount(async () => {
    const initial = await getLoadingStatus();
    loadingStatus.set(initial);

    // Wait for embedding model to finish loading, then auto-open last vault
    const unlisten = listen<string>("loading-status", async (event) => {
      loadingStatus.set(event.payload);

      if (event.payload === "" && !$currentVault) {
        try {
          const lastPath = await getLastVault();
          if (lastPath) {
            const info = await openVault(lastPath);
            currentVault.set(info);
          }
        } catch (e) {
          console.warn("Failed to auto-open last vault:", e);
        }
      }
    });

    // If already loaded (no loading status), open last vault immediately
    if (!initial) {
      try {
        const lastPath = await getLastVault();
        if (lastPath) {
          const info = await openVault(lastPath);
          currentVault.set(info);
        }
      } catch (e) {
        console.warn("Failed to auto-open last vault:", e);
      }
    }

    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

{#if $loadingStatus && $currentVault === null}
  <LoadingScreen />
{:else if $currentVault === null}
  <FolderPicker />
{:else if $currentPage === "search"}
  <SearchPage />
{:else}
  <DescriptionsPage />
{/if}

<style>
  :global(#app) {
    display: flex;
    flex-direction: column;
  }
</style>
