<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import type { Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import {
    deleteFilter,
    listFilters,
    onAlert,
    onWatcherError,
    refreshCatalog as fetchCatalog,
    updateFilter,
    type Alert,
    type CatalogStatus,
    type FilterView,
  } from "#lib/api.ts";
  import FilterForm from "#lib/FilterForm.svelte";
  import { findUpdate, installUpdate } from "#lib/updater.ts";

  let version = $state("");
  let update = $state<Update | null>(null);
  let updateStatus = $state("");

  onMount(async () => {
    version = await getVersion();
    try {
      update = await findUpdate();
    } catch (e) {
      console.error("update check failed", e);
    }
  });

  async function runUpdate() {
    if (!update) return;
    updateStatus = "Downloading...";
    try {
      await installUpdate(update, (done, total) => {
        updateStatus = total
          ? `Downloading ${Math.round((done / total) * 100)}%`
          : "Downloading...";
      });
    } catch (e) {
      updateStatus = `Update failed: ${e}`;
    }
  }

  let catalog = $state<CatalogStatus | null>(null);

  async function refreshCatalog() {
    catalog = null;
    try {
      catalog = await fetchCatalog();
    } catch (e) {
      catalog = { version: null, error: String(e) };
    }
    // names in the list come from the catalog
    await loadFilters();
  }

  let filters = $state<FilterView[]>([]);
  let filterError = $state("");
  // undefined = form closed, null = new filter
  let editing = $state<FilterView | null | undefined>(undefined);
  let confirmingDelete = $state<number | null>(null);
  let watcherError = $state("");

  async function loadFilters() {
    try {
      filters = await listFilters();
      filterError = "";
    } catch (e) {
      filterError = String(e);
    }
  }

  // every filter change restarts dead connections, so the old error is stale
  async function changed() {
    editing = undefined;
    confirmingDelete = null;
    watcherError = "";
    await loadFilters();
  }

  async function toggle(filter: FilterView) {
    try {
      await updateFilter({ ...filter, enabled: !filter.enabled });
      await changed();
    } catch (e) {
      filterError = String(e);
    }
  }

  async function remove(id: number) {
    try {
      await deleteFilter(id);
      await changed();
    } catch (e) {
      filterError = String(e);
    }
  }

  const platformNames: Record<string, string> = {
    pc: "PC",
    ps4: "PlayStation",
    xbox: "Xbox",
    switch: "Switch",
    mobile: "Mobile",
  };
  const statusNames: Record<string, string> = {
    ingame: "in game",
    online: "online+",
    offline: "anyone",
  };

  function describe(f: FilterView) {
    const name = f.item?.name ?? `unknown item ${f.itemId}`;
    const rank = f.rankMin === null ? "" : ` rank ${f.rankMin}-${f.rankMax}`;
    const subtype = f.subtype ? ` (${f.subtype})` : "";
    const price = f.intent === "buy" ? `max ${f.priceLimit}p` : `min ${f.priceLimit}p`;
    return `${f.intent === "buy" ? "Buy" : "Sell"} ${name}${rank}${subtype}, ${price}`;
  }

  function where(f: FilterView) {
    const platform = platformNames[f.platform ?? "pc"];
    const crossplay = f.crossplay ? " + crossplay" : "";
    return `${platform}${crossplay}, ${statusNames[f.minStatus ?? "ingame"]}`;
  }

  type Shown = Alert & { key: number; copied: boolean };
  let alerts = $state<Shown[]>([]);
  let alertKey = 0;

  async function copy(alert: Shown) {
    try {
      await navigator.clipboard.writeText(alert.whisper);
      alert.copied = true;
    } catch (e) {
      watcherError = `Copy failed: ${e}`;
    }
  }

  onMount(() => {
    refreshCatalog();
    const stops = [
      onAlert((alert) => {
        alerts = [{ ...alert, key: alertKey++, copied: false }, ...alerts].slice(0, 50);
      }),
      onWatcherError((message) => (watcherError = message)),
    ];
    return () => stops.forEach((stop) => stop.then((unlisten) => unlisten()));
  });
</script>

<main>
  {#if update}
    <p role="status">
      Version {update.version} is available.
      <button onclick={runUpdate} disabled={!!updateStatus}>Update now</button>
      {updateStatus}
    </p>
  {/if}
  <h1>NasuaFrame {version}</h1>

  <p role="status">
    {#if !catalog}
      Loading items...
    {:else if catalog.version}
      Items ready{catalog.error ? " (offline, using the saved list)" : ""}
    {:else}
      Can't load items: {catalog.error}
      <button onclick={refreshCatalog}>Retry</button>
    {/if}
  </p>

  {#if watcherError}
    <p class="error" role="alert">Order stream stopped: {watcherError}</p>
  {/if}

  <section>
    <div class="row heading">
      <h2>Filters</h2>
      {#if editing === undefined}
        <button onclick={() => (editing = null)} disabled={!catalog?.version}>Add filter</button>
      {/if}
    </div>

    {#if editing !== undefined}
      {#key editing}
        <FilterForm {editing} onsaved={changed} oncancel={() => (editing = undefined)} />
      {/key}
    {/if}

    {#if filterError}<p class="error" role="alert">{filterError}</p>{/if}

    {#if filters.length}
      <ul class="list">
        {#each filters as filter (filter.id)}
          <li class="card" class:off={!filter.enabled}>
            <div>
              <strong>{describe(filter)}</strong>
              <small>{where(filter)}</small>
            </div>
            <div class="row">
              <label>
                <input type="checkbox" checked={filter.enabled} onchange={() => toggle(filter)} />
                On
              </label>
              <button onclick={() => (editing = filter)}>Edit</button>
              {#if confirmingDelete === filter.id}
                <button onclick={() => remove(filter.id)}>Really delete</button>
                <button onclick={() => (confirmingDelete = null)}>Keep</button>
              {:else}
                <button onclick={() => (confirmingDelete = filter.id)}>Delete</button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {:else if editing === undefined}
      <p>No filters yet.</p>
    {/if}
  </section>

  <section>
    <h2>Matches</h2>
    {#if alerts.length}
      <ul class="list">
        {#each alerts as alert (alert.key)}
          <li class="card">
            <div>
              <strong>{alert.itemName}, {alert.platinum}p</strong>
              <small>{alert.trader}</small>
              <code>{alert.whisper}</code>
            </div>
            <button onclick={() => copy(alert)}>{alert.copied ? "Copied" : "Copy whisper"}</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p>Nothing yet, matches show up here while the app is open.</p>
    {/if}
  </section>
</main>

<style>
  :root {
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
    font-size: 16px;
    line-height: 24px;
    font-weight: 400;

    color: #0f0f0f;
    background-color: #f6f6f6;

    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    -webkit-text-size-adjust: 100%;
  }

  main {
    max-width: 44em;
    margin: 0 auto;
    padding: 1em;
  }

  h1 {
    margin-bottom: 0.2em;
  }

  .heading {
    justify-content: space-between;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.6em;
  }

  .list .card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1em;
  }

  .list .card > div:first-child {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .off {
    opacity: 0.55;
  }

  code {
    font-size: 0.85em;
    overflow-wrap: anywhere;
  }

  :global(.card) {
    background: #ffffff;
    border-radius: 8px;
    padding: 0.8em 1em;
    box-shadow: 0 2px 2px rgba(0, 0, 0, 0.1);
  }

  :global(.row) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6em;
  }

  :global(.error) {
    color: #c62828;
  }

  :global(input:not([type="checkbox"], [type="radio"])),
  :global(select),
  :global(button) {
    border-radius: 8px;
    border: 1px solid transparent;
    padding: 0.5em 1em;
    font-size: 1em;
    font-family: inherit;
    color: #0f0f0f;
    background-color: #ffffff;
    box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
  }

  :global(button) {
    cursor: pointer;
    font-weight: 500;
  }

  :global(button:hover:not(:disabled)) {
    border-color: #396cd8;
  }

  :global(button:disabled) {
    cursor: default;
    opacity: 0.6;
  }

  :global(:focus-visible) {
    outline: 2px solid #396cd8;
    outline-offset: 2px;
  }

  @media (prefers-color-scheme: dark) {
    :root {
      color: #f6f6f6;
      background-color: #2f2f2f;
    }

    :global(.card) {
      background: #3a3a3a;
    }

    :global(.error) {
      color: #ff8a80;
    }

    :global(input:not([type="checkbox"], [type="radio"])),
    :global(select),
    :global(button) {
      color: #ffffff;
      background-color: #0f0f0f98;
    }
  }
</style>
