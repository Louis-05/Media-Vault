<script lang="ts">
  import { currentPage, autoSearch } from "../stores/vault";
  import { deepRefresh, getBuildInfo } from "../api";
  import { onMount } from "svelte";

  let refreshing = $state(false);
  let version = $state("");
  let buildDate = $state("");

  onMount(async () => {
    try {
      const [v, d] = await getBuildInfo();
      version = v;
      buildDate = d;
    } catch (e) {
      console.error("Failed to get build info:", e);
    }
  });

  async function handleDeepRefresh() {
    refreshing = true;
    try {
      await deepRefresh();
    } catch (e) {
      console.error("Deep refresh failed:", e);
    }
    refreshing = false;
  }

  function goBack() {
    currentPage.set("search");
  }
</script>

<div class="settings-page">
  <header>
    <button class="secondary" onclick={goBack}>Back</button>
    <h2>Settings</h2>
  </header>

  <main>
    <section>
      <h3>Search</h3>
      <div class="setting-row">
        <div class="setting-info">
          <strong>Automatic search</strong>
          <p>Search as you type. When off, press Enter or click the search button.</p>
        </div>
        <label class="toggle">
          <input type="checkbox" bind:checked={$autoSearch} />
          <span class="slider"></span>
        </label>
      </div>
    </section>

    <section>
      <h3>Vault Maintenance</h3>
      <div class="setting-row">
        <div class="setting-info">
          <strong>Deep Refresh</strong>
          <p>Scan for new files, clean up missing entries, sync descriptions, and detect stale embeddings.</p>
        </div>
        <button onclick={handleDeepRefresh} disabled={refreshing}>
          {refreshing ? "Refreshing..." : "Deep Refresh"}
        </button>
      </div>
    </section>

    <section class="build-info">
      <p>Media Vault v{version} — built {buildDate}</p>
    </section>
  </main>
</div>

<style>
  .settings-page {
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
    max-width: 600px;
  }

  section {
    margin-bottom: 32px;
  }

  h3 {
    margin: 0 0 16px 0;
    font-size: 0.95rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }

  .setting-info {
    flex: 1;
  }

  .setting-info strong {
    display: block;
    color: var(--text);
    margin-bottom: 4px;
  }

  .setting-info p {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .toggle {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
    flex-shrink: 0;
  }

  .toggle input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background-color: var(--border);
    border-radius: 24px;
    transition: background-color 0.2s;
  }

  .slider::before {
    content: "";
    position: absolute;
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background-color: var(--bg);
    border-radius: 50%;
    transition: transform 0.2s;
  }

  .toggle input:checked + .slider {
    background-color: var(--accent);
  }

  .toggle input:checked + .slider::before {
    transform: translateX(20px);
  }

  .build-info {
    margin-top: auto;
    padding-top: 24px;
    border-top: 1px solid var(--border);
  }

  .build-info p {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-muted);
  }
</style>
