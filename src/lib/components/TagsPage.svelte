<script lang="ts">
  import { currentPage } from "../stores/vault";
  import { getAllTagKeys, getTagValues, createTagKey, renameTagKey, renameTagValue, deleteTagKey } from "../api";
  import type { TagKeyInfo } from "../api";
  import { onMount } from "svelte";

  let tagKeys = $state<TagKeyInfo[]>([]);
  let expandedKey = $state<string | null>(null);
  let expandedValues = $state<string[]>([]);
  let newKeyName = $state("");
  let renamingKey = $state<string | null>(null);
  let renameValue = $state("");
  let confirmDeleteKey = $state<string | null>(null);

  // Right-click context menu for tag values
  let valueMenu = $state<{ x: number; y: number; value: string } | null>(null);
  // Inline rename state for a tag value
  let renamingValue = $state<string | null>(null);
  let renameValueInput = $state("");

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

  function openValueMenu(e: MouseEvent, value: string) {
    e.preventDefault();
    valueMenu = { x: e.clientX, y: e.clientY, value };
  }

  function closeValueMenu() {
    valueMenu = null;
  }

  function startRenameValue(value: string) {
    renamingValue = value;
    renameValueInput = value;
    valueMenu = null;
  }

  async function finishRenameValue() {
    if (!expandedKey || renamingValue === null) {
      renamingValue = null;
      return;
    }
    const newVal = renameValueInput.trim();
    if (!newVal || newVal === renamingValue) {
      renamingValue = null;
      return;
    }
    const key = expandedKey;
    const oldVal = renamingValue;
    try {
      await renameTagValue(key, oldVal, newVal);
      renamingValue = null;
      // Reload values for the currently expanded key
      expandedValues = await getTagValues(key);
    } catch (e) {
      console.error("Failed to rename tag value:", e);
      renamingValue = null;
    }
  }

  function goBack() {
    currentPage.set("search");
  }
</script>

<svelte:window onclick={closeValueMenu} />


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
                  {#if renamingValue === value}
                    <input
                      class="value-rename-input"
                      type="text"
                      bind:value={renameValueInput}
                      onkeydown={(e) => { if (e.key === "Enter") finishRenameValue(); if (e.key === "Escape") renamingValue = null; }}
                      onblur={finishRenameValue}
                    />
                  {:else}
                    <span
                      class="value-chip"
                      role="button"
                      tabindex="0"
                      oncontextmenu={(e) => openValueMenu(e, value)}
                    >{value}</span>
                  {/if}
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

  {#if valueMenu}
    <div
      class="context-menu"
      style="left: {valueMenu.x}px; top: {valueMenu.y}px;"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      oncontextmenu={(e) => e.preventDefault()}
      role="menu"
      tabindex="-1"
    >
      <button class="context-menu-item" onclick={() => startRenameValue(valueMenu!.value)}>Rename</button>
    </div>
  {/if}
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
    cursor: context-menu;
    user-select: none;
  }

  .value-chip:hover {
    border-color: var(--accent);
  }

  .value-rename-input {
    padding: 2px 8px;
    font-size: 0.8rem;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    min-width: 80px;
  }

  .context-menu {
    position: fixed;
    z-index: 1000;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    min-width: 120px;
  }

  .context-menu-item {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: var(--text);
    font-size: 0.85rem;
    padding: 6px 10px;
    border-radius: 4px;
    cursor: pointer;
  }

  .context-menu-item:hover {
    background: var(--bg);
    color: var(--accent);
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
