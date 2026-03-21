<script lang="ts">
  interface Props {
    onSearch: (query: string) => void;
  }
  let { onSearch }: Props = $props();
  let query = $state("");

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    query = target.value;
    onSearch(query);
  }
</script>

<div class="search-bar">
  <input
    type="text"
    placeholder="Search media by description..."
    value={query}
    oninput={handleInput}
  />
</div>

<style>
  .search-bar {
    width: 100%;
  }

  input {
    width: 100%;
    font-size: 1rem;
    padding: 10px 16px;
    border-radius: 8px;
  }
</style>
