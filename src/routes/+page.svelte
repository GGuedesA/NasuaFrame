<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { Update } from "@tauri-apps/plugin-updater";
  import { onMount } from "svelte";
  import {
    deleteFilter,
    getSettings,
    listBestOrders,
    listFilters,
    onBestOrders,
    onFeedStatus,
    openLogFolder,
    refreshCatalog as fetchCatalog,
    updateFilter,
    type BestOrder,
    type CatalogStatus,
    type FilterView,
  } from "#lib/api.ts";
  import FilterForm from "#lib/FilterForm.svelte";
  import { setLanguage, t, type Key } from "#lib/i18n.svelte.ts";
  import SettingsForm from "#lib/SettingsForm.svelte";
  import Startup from "#lib/Startup.svelte";
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
    updateStatus = t("update.downloading");
    try {
      await installUpdate(update, (done, total) => {
        updateStatus = total
          ? t("update.progress", { percent: Math.round((done / total) * 100) })
          : t("update.downloading");
      });
    } catch (e) {
      updateStatus = t("update.failed", { error: String(e) });
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
  let feedProblems = $state<Record<string, string>>({});
  let actionError = $state("");

  let filtersLoaded = $state(false);
  let settingsLoaded = $state<boolean | null>(null);
  let starting = $state(true);

  async function loadFilters() {
    try {
      filters = await listFilters();
      filtersLoaded = true;
      filterError = "";
    } catch (e) {
      filterError = String(e);
    }
  }

  async function changed() {
    editing = undefined;
    confirmingDelete = null;
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

  function describe(f: FilterView) {
    return t(f.intent === "buy" ? "filters.buy" : "filters.sell", {
      item: f.item?.name ?? t("filters.unknownItem", { id: f.itemId }),
      rank: f.rankMin === null ? "" : t("filters.rank", { min: f.rankMin, max: f.rankMax ?? "" }),
      subtype: f.subtype ? ` (${f.subtype})` : "",
      price: f.priceLimit,
    });
  }

  function where(f: FilterView) {
    const platform = t(`platform.${f.platform ?? "pc"}` as Key);
    const crossplay = f.crossplay ? ` + ${t("crossplay")}` : "";
    return `${platform}${crossplay}, ${t(`minStatus.${f.minStatus ?? "ingame"}` as Key)}`;
  }

  function details(order: BestOrder) {
    const status = t(`status.${order.status}` as Key);
    return order.rank === null ? status : t("orders.rank", { status, rank: order.rank });
  }

  let showSettings = $state(false);
  let best = $state<Record<number, BestOrder[]>>({});
  let copied = $state("");

  async function copy(order: BestOrder) {
    try {
      await navigator.clipboard.writeText(order.whisper);
      copied = order.id;
    } catch (e) {
      actionError = t("orders.copyFailed", { error: String(e) });
    }
  }

  async function showLogs() {
    try {
      await openLogFolder();
    } catch (e) {
      actionError = t("logs.failed", { error: String(e) });
    }
  }

  onMount(() => {
    getSettings()
      .then((s) => {
        setLanguage(s.language);
        settingsLoaded = true;
      })
      .catch((e) => {
        settingsLoaded = false;
        console.error("settings not loaded", e);
      });
    refreshCatalog();
    const stops = [
      onBestOrders(({ filterId, orders }) => {
        if (orders) best[filterId] = orders;
        else delete best[filterId];
      }),
      onFeedStatus(({ feed, problem }) => {
        if (problem) feedProblems[feed] = problem;
        else delete feedProblems[feed];
      }),
    ];
    listBestOrders()
      .then((lists) => (best = { ...lists, ...best }))
      .catch((e) => (actionError = t("orders.loadFailed", { error: String(e) })));
    return () => stops.forEach((stop) => stop.then((unlisten) => unlisten()));
  });
  const appWindow = getCurrentWindow();
</script>

{#if starting}
  <Startup
    settings={settingsLoaded}
    {catalog}
    enabledFilters={filtersLoaded ? filters.filter((f) => f.enabled).length : null}
    feedProblem={Object.keys(feedProblems).length > 0}
    ondone={() => (starting = false)}
  />
{/if}

<!-- close still goes through CloseRequested for the tray -->
<div class="titlebar" data-tauri-drag-region>
  <span data-tauri-drag-region>NasuaFrame</span>
  <div class="controls">
    <button aria-label={t("window.minimize")} onclick={() => appWindow.minimize()}>&#x2013;</button>
    <button aria-label={t("window.maximize")} onclick={() => appWindow.toggleMaximize()}
      >&#x25A1;</button
    >
    <button class="close" aria-label={t("window.close")} onclick={() => appWindow.close()}
      >&#x2715;</button
    >
  </div>
</div>

<main>
  {#if update}
    <p role="status">
      {t("update.available", { version: update.version })}
      <button onclick={runUpdate} disabled={!!updateStatus}>{t("update.install")}</button>
      {updateStatus}
    </p>
  {/if}
  <div class="row heading">
    <h1>NasuaFrame {version}</h1>
    {#if !showSettings}
      <button onclick={() => (showSettings = true)}>{t("settings.open")}</button>
    {/if}
  </div>

  {#if showSettings}
    <SettingsForm
      onsaved={() => {
        showSettings = false;
        refreshCatalog();
      }}
      oncancel={() => (showSettings = false)}
    />
  {/if}

  <p role="status">
    {#if !catalog}
      {t("catalog.loading")}
    {:else if catalog.version}
      {t(catalog.error ? "catalog.offline" : "catalog.ready")}
    {:else}
      {t("catalog.failed", { error: catalog.error ?? "" })}
      <button onclick={refreshCatalog}>{t("catalog.retry")}</button>
    {/if}
  </p>

  {#each Object.entries(feedProblems) as [feed, problem] (feed)}
    <p class="error" role="status">{t("feed.problem", { feed, problem })}</p>
  {/each}
  {#if actionError}
    <p class="error" role="alert">{actionError}</p>
  {/if}

  <section>
    <div class="row heading">
      <h2>{t("filters.title")}</h2>
      {#if editing === undefined}
        <button onclick={() => (editing = null)} disabled={!catalog?.version}
          >{t("filters.add")}</button
        >
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
            <div class="line">
              <div class="text">
                <strong>{describe(filter)}</strong>
                <small>{where(filter)}</small>
              </div>
              <div class="row">
                <label>
                  <input type="checkbox" checked={filter.enabled} onchange={() => toggle(filter)} />
                  {t("filters.on")}
                </label>
                <button onclick={() => (editing = filter)}>{t("filters.edit")}</button>
                {#if confirmingDelete === filter.id}
                  <button onclick={() => remove(filter.id)}>{t("filters.reallyDelete")}</button>
                  <button onclick={() => (confirmingDelete = null)}>{t("filters.keep")}</button>
                {:else}
                  <button onclick={() => (confirmingDelete = filter.id)}
                    >{t("filters.delete")}</button
                  >
                {/if}
              </div>
            </div>
            {#if filter.enabled}
              {@const orders = best[filter.id]}
              {#if !orders}
                <small>{t("orders.looking")}</small>
              {:else if orders.length}
                <ul class="orders">
                  {#each orders as order (order.id)}
                    <li class="line">
                      <div class="text">
                        <span>{order.platinum}p, {order.trader}</span>
                        <small>{details(order)}</small>
                      </div>
                      <button onclick={() => copy(order)}>
                        {t(copied === order.id ? "orders.copied" : "orders.copy")}
                      </button>
                    </li>
                  {/each}
                </ul>
              {:else}
                <small>{t("orders.none")}</small>
              {/if}
            {/if}
          </li>
        {/each}
      </ul>
    {:else if editing === undefined}
      <p>{t("filters.none")}</p>
    {/if}
  </section>

  <footer>
    <button onclick={showLogs}>{t("logs.open")}</button>
  </footer>
</main>

<style>
  .titlebar {
    position: sticky;
    top: 0;
    z-index: 10;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 0.8em;
    background: var(--titlebar-bg);
    color: var(--titlebar-text);
    font-size: 13px;
    user-select: none;
  }

  .titlebar .controls {
    display: flex;
    height: 100%;
  }

  .titlebar .controls button {
    width: 46px;
    height: 100%;
    border: none;
    border-radius: 0;
    box-shadow: none;
    background: transparent;
    color: inherit;
    padding: 0;
  }

  .titlebar .controls button:hover {
    background: var(--titlebar-hover);
  }

  .titlebar .controls .close:hover {
    background: var(--close-hover);
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
    flex-direction: column;
    gap: 0.6em;
  }

  .line {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1em;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .orders {
    list-style: none;
    margin: 0;
    padding: 0.6em 0 0;
    border-top: 1px solid var(--divider);
    display: flex;
    flex-direction: column;
    gap: 0.4em;
  }

  footer {
    margin-top: 2em;
  }

  .off {
    opacity: 0.55;
  }
</style>
