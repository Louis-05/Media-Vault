<script lang="ts">
  import { untrack } from "svelte";

  interface Props {
    onSearch: (query: string) => void;
    auto?: boolean;
    initialQuery?: string;
  }
  let { onSearch, auto = true, initialQuery = "" }: Props = $props();
  // Only the initial value is used; later changes to the prop are ignored.
  let query = $state(untrack(() => initialQuery));
  let debounceTimeout: ReturnType<typeof setTimeout> | undefined;

  function handleInput() {
    if (auto) {
      if (debounceTimeout) clearTimeout(debounceTimeout);
      debounceTimeout = setTimeout(() => onSearch(query), 300);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (debounceTimeout) clearTimeout(debounceTimeout);
      onSearch(query);
    }
  }

  function handleClick() {
    if (debounceTimeout) clearTimeout(debounceTimeout);
    onSearch(query);
  }
</script>

<div class="search-bar">
  <input
    type="text"
    placeholder="Search media by description..."
    bind:value={query}
    oninput={handleInput}
    onkeydown={handleKeydown}
  />
  {#if !auto}
    <button class="search-btn" onclick={handleClick} title="Search">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="8" />
        <line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
    </button>
  {/if}
</div>

<style>
  .search-bar {
    width: 100%;
    display: flex;
    gap: 0;
  }

  input {
    flex: 1;
    font-size: 1rem;
    padding: 10px 16px;
    border-radius: 8px;
  }

  .search-bar:has(.search-btn) input {
    border-radius: 8px 0 0 8px;
    border-right: none;
  }

  .search-btn {
    padding: 10px 14px;
    border-radius: 0 8px 8px 0;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .search-btn:hover {
    background: var(--bg);
    color: var(--text);
  }
</style>
