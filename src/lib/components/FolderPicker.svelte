<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { createVault, openVault } from "../api";
  import { currentVault } from "../stores/vault";

  let error = $state("");

  async function handleOpen() {
    const selected = await open({ directory: true });
    if (!selected) return;

    try {
      const info = await openVault(selected);
      currentVault.set(info);
    } catch {
      // Not a vault yet, try creating
      try {
        const info = await createVault(selected);
        currentVault.set(info);
      } catch (e) {
        error = `Failed to open/create vault: ${e}`;
      }
    }
  }
</script>

<div class="picker">
  <div class="content">
    <h1>Media Vault</h1>
    <p>Search your media by description using AI embeddings.</p>
    <button class="primary" onclick={handleOpen}>
      Open or Create Vault
    </button>
    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>
</div>

<style>
  .picker {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    text-align: center;
  }

  .content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }

  h1 {
    font-size: 2rem;
    color: var(--accent);
  }

  p {
    color: var(--text-muted);
  }

  .error {
    color: #ff4444;
    font-size: 0.9em;
  }

  button {
    font-size: 1.1rem;
    padding: 12px 32px;
  }
</style>
