<script lang="ts">
  import { currentPage } from "../stores/vault";
  import { getAllTagKeys, getTagValues, createTagKey, renameTagKey, deleteTagKey } from "../api";
  import type { TagKeyInfo } from "../api";
  import { onMount } from "svelte";

  let tagKeys = $state<TagKeyInfo[]>([]);
  let expandedKey = $state<string | null>(null);
  let expandedValues = $state<string[]>([]);
  let newKeyName = $state("");
  let renamingKey = $state<string | null>(null);
  let renameValue = $state("");
  let confirmDeleteKey = $state<string | null>(null);

  onMount(() => {
    loadKeys();
  });

  async function loadKeys() {
    try {
      tagKeys = await getAllTagKeys();
    } catch (e) {
      console.error("Failed to load tag keys:", e);
    }
  }

  async function toggleExpand(key: string) {
    if (expandedKey === key) {
      expandedKey = null;
      expandedValues = [];
    } else {
      expandedKey = key;
      try {
        expandedValues = await getTagValues(key);
      } catch (e) {
        console.error("Failed to load values:", e);
        expandedValues = [];
      }
    }
  }

  async function handleCreateKey() {
    const key = newKeyName.trim();
    if (!key) return;
    try {
      await createTagKey(key);
      newKeyName = "";
      await loadKeys();
    } catch (e) {
      console.error("Failed to create tag key:", e);
    }
  }

  function startRename(key: string) {
    renamingKey = key;
    renameValue = key;
  }

  async function finishRename() {
    if (!renamingKey || !renameValue.trim() || renameValue.trim() === renamingKey) {
      renamingKey = null;
      return;
    }
    try {
      await renameTagKey(renamingKey, renameValue.trim());
      renamingKey = null;
      await loadKeys();
    } catch (e) {
      console.error("Failed to rename tag:", e);
    }
  }

  async function handleDeleteKey(key: string) {
    try {
      await deleteTagKey(key);
      confirmDeleteKey = null;
      if (expandedKey === key) {
        expandedKey = null;
        expandedValues = [];
      }
      await loadKeys();
    } catch (e) {
      console.error("Failed to delete tag:", e);
    }
  }

  function goBack() {
    currentPage.set("search");
  }
</script>

<div class="tags-page">
  <header>
    <button class="secondary" onclick={goBack}>← Back</button>
    <h2>Tag Management</h2>
  </header>

  <main>
    <div class="create-key">
      <input
        type="text"
        placeholder="New tag key..."
        bind:value={newKeyName}
        onkeydown={(e) => { if (e.key === "Enter") handleCreateKey(); }}
      />
      <button onclick={handleCreateKey} disabled={!newKeyName.trim()}>Create</button>
    </div>

    <div class="key-list">
      {#each tagKeys.filter(k => k.key !== "media_type" && k.key !== "description") as tagKey}
        <div class="key-item" class:expanded={expandedKey === tagKey.key}>
          <div class="key-row">
            {#if renamingKey === tagKey.key}
              <input
                class="rename-input"
                type="text"
                bind:value={renameValue}
                onkeydown={(e) => { if (e.key === "Enter") finishRename(); if (e.key === "Escape") renamingKey = null; }}
                onblur={finishRename}
              />
            {:else}
              <button class="key-name" onclick={() => toggleExpand(tagKey.key)}>
                <span class="expand-icon">{expandedKey === tagKey.key ? "▼" : "▶"}</span>
                {tagKey.key}
              </button>
            {/if}
            <span class="key-count">{tagKey.usage_count} media</span>
            <div class="key-actions">
              <button class="action-sm" onclick={() => startRename(tagKey.key)} title="Rename">Rename</button>
              {#if confirmDeleteKey === tagKey.key}
                <button class="action-sm delete-confirm-btn" onclick={() => handleDeleteKey(tagKey.key)}>Confirm</button>
                <button class="action-sm" onclick={() => confirmDeleteKey = null}>Cancel</button>
              {:else}
                <button class="action-sm delete-action" onclick={() => confirmDeleteKey = tagKey.key} title="Delete">Delete</button>
              {/if}
            </div>
          </div>

          {#if expandedKey === tagKey.key}
            <div class="values-list">
              {#if expandedValues.length === 0}
                <span class="no-values">No values yet</span>
              {:else}
                {#each expandedValues as value}
                  <span class="value-chip">{value}</span>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}

      {#if tagKeys.length === 0}
        <div class="empty">No tags created yet.</div>
      {/if}
    </div>
  </main>
</div>

<style>
  .tags-page {
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
    gap: 12px;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
    color: var(--text);
  }

  main {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    max-width: 700px;
  }

  .create-key {
    display: flex;
    gap: 8px;
    margin-bottom: 24px;
  }

  .create-key input {
    flex: 1;
    padding: 8px 12px;
    font-size: 0.9rem;
    border-radius: 6px;
  }

  .create-key button {
    padding: 8px 16px;
    font-size: 0.9rem;
  }

  .key-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .key-item {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }

  .key-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
  }

  .key-name {
    flex: 1;
    text-align: left;
    background: none;
    border: none;
    color: var(--text);
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    padding: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .key-name:hover {
    color: var(--accent);
  }

  .expand-icon {
    font-size: 0.65rem;
    color: var(--text-muted);
  }

  .rename-input {
    flex: 1;
    padding: 4px 8px;
    font-size: 0.9rem;
    border-radius: 4px;
  }

  .key-count {
    color: var(--text-muted);
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .key-actions {
    display: flex;
    gap: 4px;
  }

  .action-sm {
    padding: 2px 8px;
    font-size: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }

  .action-sm:hover {
    background: var(--bg);
    color: var(--text);
  }

  .delete-action {
    color: #ff4444 !important;
    border-color: #ff4444 !important;
  }

  .delete-action:hover {
    background: rgba(255, 68, 68, 0.1) !important;
  }

  .delete-confirm-btn {
    background: #ff4444 !important;
    color: white !important;
    border-color: #ff4444 !important;
  }

  .values-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 12px 12px 28px;
    border-top: 1px solid var(--border);
  }

  .value-chip {
    padding: 2px 8px;
    font-size: 0.8rem;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
  }

  .no-values {
    font-size: 0.8rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .empty {
    text-align: center;
    padding: 40px;
    color: var(--text-muted);
  }
</style>
