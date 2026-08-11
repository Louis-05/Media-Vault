<script lang="ts">
  import { currentPage } from "../stores/vault";
  import { getLogsSince, clearLogs, copyLogs, openLogsFolder, type LogEntry } from "../api";
  import { onMount } from "svelte";

  /** Must match CAPACITY in src-tauri/src/log_buffer.rs. */
  const MAX_ENTRIES = 5000;
  /** Cap on rendered rows — the buffer can hold far more than is useful to show. */
  const MAX_RENDERED = 2000;
  const POLL_MS = 750;

  const LEVEL_RANK: Record<string, number> = { TRACE: 0, DEBUG: 1, INFO: 2, WARN: 3, ERROR: 4 };

  let entries = $state<LogEntry[]>([]);
  let lastSeq = 0;
  let minLevel = $state("all");
  let search = $state("");
  let autoScroll = $state(true);
  let copied = $state(false);
  let logEl = $state<HTMLDivElement | null>(null);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const filtered = $derived.by(() => {
    const minRank = minLevel === "all" ? -1 : (LEVEL_RANK[minLevel] ?? -1);
    const needle = search.trim().toLowerCase();
    return entries.filter((e) => {
      if ((LEVEL_RANK[e.level] ?? 0) < minRank) return false;
      if (!needle) return true;
      return (
        e.message.toLowerCase().includes(needle) || e.target.toLowerCase().includes(needle)
      );
    });
  });

  const visible = $derived(
    filtered.length > MAX_RENDERED ? filtered.slice(-MAX_RENDERED) : filtered,
  );

  onMount(() => {
    let stopped = false;

    async function poll() {
      try {
        const fresh = await getLogsSince(lastSeq);
        if (stopped || fresh.length === 0) return;
        lastSeq = fresh[fresh.length - 1].seq;
        const next = entries.concat(fresh);
        entries = next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next;
      } catch (e) {
        console.error("Failed to fetch logs:", e);
      }
    }

    poll();
    const timer = setInterval(poll, POLL_MS);

    return () => {
      stopped = true;
      clearInterval(timer);
      if (copyTimer) clearTimeout(copyTimer);
    };
  });

  // Follow the tail as new lines arrive, unless the user has scrolled away.
  $effect(() => {
    void visible;
    if (autoScroll && logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  function handleScroll() {
    if (!logEl) return;
    const distanceFromBottom = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight;
    autoScroll = distanceFromBottom <= 40;
  }

  function shortTarget(target: string): string {
    return target.replace(/^media_vault_lib::/, "");
  }

  function shortTime(timestamp: string): string {
    const space = timestamp.indexOf(" ");
    return space === -1 ? timestamp : timestamp.slice(space + 1);
  }

  async function handleCopy() {
    const text = filtered
      .map((e) => `${e.timestamp} ${e.level} ${e.target} ${e.message}`)
      .join("\n");
    try {
      await copyLogs(text);
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1500);
    } catch (e) {
      console.error("Failed to copy logs:", e);
    }
  }

  async function handleClear() {
    try {
      await clearLogs();
      entries = [];
    } catch (e) {
      console.error("Failed to clear logs:", e);
    }
  }

  async function handleOpenFolder() {
    try {
      await openLogsFolder();
    } catch (e) {
      console.error("Failed to open logs folder:", e);
    }
  }

  function goBack() {
    currentPage.set("settings");
  }
</script>

<div class="logs-page">
  <header>
    <button class="secondary" onclick={goBack}>Back</button>
    <h2>Logs</h2>

    <div class="controls">
      <select bind:value={minLevel} aria-label="Minimum log level">
        <option value="all">All levels</option>
        <option value="INFO">Info &amp; up</option>
        <option value="WARN">Warn &amp; up</option>
        <option value="ERROR">Errors only</option>
      </select>

      <input type="text" placeholder="Filter..." bind:value={search} />

      <label class="autoscroll" title="Follow new log lines as they arrive">
        <input type="checkbox" bind:checked={autoScroll} />
        Auto-scroll
      </label>

      <button onclick={handleCopy} disabled={filtered.length === 0}>
        {copied ? "Copied!" : "Copy"}
      </button>
      <button onclick={handleClear} title="Empties this view. The log file on disk is not affected.">
        Clear
      </button>
      <button onclick={handleOpenFolder}>Open Folder</button>
    </div>
  </header>

  {#if filtered.length > visible.length}
    <p class="truncated">Showing the last {visible.length} of {filtered.length} lines.</p>
  {/if}

  <div class="log-view" bind:this={logEl} onscroll={handleScroll}>
    {#if visible.length === 0}
      <p class="empty">
        {entries.length === 0 ? "No log messages yet." : "No lines match the current filter."}
      </p>
    {:else}
      {#each visible as entry (entry.seq)}
        <div class="row">
          <span class="ts">{shortTime(entry.timestamp)}</span>
          <span class="level level-{entry.level.toLowerCase()}">{entry.level}</span>
          <span class="target">{shortTarget(entry.target)}</span>
          <span class="msg">{entry.message}</span>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .logs-page {
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
    flex-shrink: 0;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
    color: var(--text);
  }

  .controls {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  select,
  .controls input[type="text"] {
    background-color: var(--input-bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 0.85rem;
  }

  .controls input[type="text"] {
    width: 160px;
  }

  select:focus,
  .controls input[type="text"]:focus {
    outline: none;
    border-color: var(--accent);
  }

  .autoscroll {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
    color: var(--text-muted);
    cursor: pointer;
    user-select: none;
  }

  .autoscroll input {
    cursor: pointer;
    accent-color: var(--accent);
  }

  .truncated {
    margin: 0;
    padding: 6px 16px;
    font-size: 0.75rem;
    color: var(--text-muted);
    background-color: var(--bg-surface);
    border-bottom: 1px solid var(--border);
  }

  .log-view {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
    font-family: ui-monospace, "Cascadia Code", "Consolas", monospace;
    font-size: 0.8rem;
    line-height: 1.5;
  }

  .row {
    display: flex;
    gap: 10px;
    padding: 1px 16px;
  }

  .row:nth-child(even) {
    background-color: rgba(255, 255, 255, 0.02);
  }

  .ts {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .level {
    flex-shrink: 0;
    width: 44px;
    font-weight: 600;
  }

  .level-error {
    color: #ff6b6b;
  }

  .level-warn {
    color: #e0b046;
  }

  .level-info,
  .level-debug,
  .level-trace {
    color: var(--text-muted);
  }

  .target {
    color: var(--text-muted);
    flex-shrink: 0;
    width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .msg {
    color: var(--text);
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
    min-width: 0;
  }

  .empty {
    padding: 32px 16px;
    text-align: center;
    color: var(--text-muted);
    font-family: system-ui, sans-serif;
    font-size: 0.9rem;
  }
</style>
