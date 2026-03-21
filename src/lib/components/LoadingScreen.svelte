<script lang="ts">
  import { loadingStatus } from "../stores/vault";
</script>

<div class="loading">
  <div class="content">
    <h1>Media Vault</h1>
    <div class="spinner"></div>
    <p>{$loadingStatus}</p>
  </div>
</div>

<style>
  .loading {
    height: 100vh;
    width: 100%;
    background-color: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    text-align: center;
  }

  h1 {
    font-size: 2rem;
    color: var(--accent);
    margin-bottom: 8px;
  }

  p {
    color: var(--text-muted);
    font-size: 0.95rem;
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 4px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
