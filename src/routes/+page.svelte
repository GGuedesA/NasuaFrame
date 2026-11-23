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
  import { deal } from "#lib/deal.ts";
  import FilterForm from "#lib/FilterForm.svelte";
  import { setLanguage, t, type Key } from "#lib/i18n.svelte.ts";
  import Ornament from "#lib/Ornament.svelte";
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
  let problems = $derived(Object.entries(feedProblems));
  let watched = $derived(filters.filter((f) => f.enabled).length);

  let selected = $state<number | null>(null);
  let current = $derived(filters.find((f) => f.id === selected) ?? filters[0]);
  let pane = $derived(showSettings ? "settings" : editing !== undefined ? "form" : "readout");

  function track(f: FilterView) {
    if (!f.enabled) return "idle";
    return best[f.id]?.length ? "lock" : "scanning";
  }

  const pad = (n: number) => String(n).padStart(2, "0");

  const appWindow = getCurrentWindow();
</script>

{#if starting}
  <Startup
    settings={settingsLoaded}
    {catalog}
    enabledFilters={filtersLoaded ? watched : null}
    feedProblem={problems.length > 0}
    ondone={() => (starting = false)}
  />
{/if}

<!-- close still goes through CloseRequested for the tray -->
<div class="titlebar" data-tauri-drag-region>
  <span class="brand" data-tauri-drag-region>
    <i class="led" class:bad={problems.length > 0}></i>
    NASUAFRAME <small>{version}</small>
  </span>
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

<div class="cockpit">
  <Ornament name="symbols" />
  <Ornament name="topright" />
  <Ornament name="sidediv" />
  <Ornament name="sidelow" />
  <Ornament name="botleft" />
  <Ornament name="botright" />

  <div class="bus">
    <div class="cell shrink" role="status">
      <span class="label">{t("startup.catalog")}</span>
      {#if !catalog}
        <span class="value">{t("catalog.loading")}</span>
      {:else if catalog.version}
        <span class="value ok">{t(catalog.error ? "catalog.offline" : "catalog.ready")}</span>
      {:else}
        {@const failed = t("catalog.failed", { error: catalog.error ?? "" })}
        <span class="value bad" title={failed}>{failed}</span>
        <button onclick={refreshCatalog}>{t("catalog.retry")}</button>
      {/if}
    </div>
    <div class="cell market" role="status">
      <Ornament name="topdiv" />
      <span class="label">{t("startup.market")}</span>
      {#if problems.length}
        <span class="value bad">{t("startup.feedProblem")}</span>
      {:else}
        <span class="value ok">{t("startup.watching", { count: watched })}</span>
      {/if}
    </div>
    {#if update}
      <div class="cell shrink" role="status">
        <span class="value ok">{t("update.available", { version: update.version })}</span>
        <button class="primary" onclick={runUpdate} disabled={!!updateStatus}
          >{t("update.install")}</button
        >
        <span class="value">{updateStatus}</span>
      </div>
    {/if}
    <button class="settings" class:on={showSettings} onclick={() => (showSettings = !showSettings)}
      >{t("settings.open")}</button
    >
  </div>

  <aside class="rail">
    <div class="rail-head">
      <h2>{t("filters.title")}</h2>
      <span class="count">{pad(filters.length)}</span>
      <button
        class="primary add"
        aria-label={t("filters.add")}
        title={t("filters.add")}
        onclick={() => {
          showSettings = false;
          editing = null;
        }}
        disabled={!catalog?.version}>+</button
      >
    </div>

    {#if filters.length}
      <ul class="slots">
        {#each filters as filter, i (filter.id)}
          {@const state = track(filter)}
          <li>
            <button
              class="slot {state}"
              class:current={current?.id === filter.id && pane === "readout"}
              aria-current={current?.id === filter.id}
              onclick={() => {
                selected = filter.id;
                editing = undefined;
                showSettings = false;
              }}
            >
              <span class="code">S-{pad(i + 1)}</span>
              <span class="name"
                >{filter.item?.name ?? t("filters.unknownItem", { id: filter.itemId })}</span
              >
              <span class="limit">
                {t(filter.intent === "buy" ? "slot.buy" : "slot.sell", {
                  price: filter.priceLimit,
                })}
              </span>
              <span class="state">
                {#if state === "scanning"}<i class="scan"></i>{/if}
                {t(`slot.${state}`)}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="hint">{t("filters.none")}</p>
    {/if}

    <button class="logs" onclick={showLogs}>{t("logs.open")}</button>
  </aside>

  <section class="pane">
    {#each problems as [feed, problem] (feed)}
      <p class="error" role="status">{t("feed.problem", { feed, problem })}</p>
    {/each}
    {#if actionError}<p class="error" role="alert">{actionError}</p>{/if}
    {#if filterError}<p class="error" role="alert">{filterError}</p>{/if}

    {#if pane === "settings"}
      <SettingsForm
        onsaved={() => {
          showSettings = false;
          refreshCatalog();
        }}
        oncancel={() => (showSettings = false)}
      />
    {:else if editing !== undefined}
      {#key editing}
        <FilterForm {editing} onsaved={changed} oncancel={() => (editing = undefined)} />
      {/key}
    {:else if current}
      {@const filter = current}
      {@const state = track(filter)}
      {@const orders = best[filter.id]}
      <div class="readout {state}">
        <header>
          <span class="code">S-{pad(filters.indexOf(filter) + 1)}</span>
          <div class="text">
            <strong>{describe(filter)}</strong>
            <small>{where(filter)}</small>
          </div>
          <span class="tag">{t(`slot.${state}`)}</span>
        </header>

        <div class="actions">
          <input
            type="checkbox"
            class="switch"
            aria-label={t("filters.on")}
            checked={filter.enabled}
            onchange={() => toggle(filter)}
          />
          <button onclick={() => (editing = filter)}>{t("filters.edit")}</button>
          {#if confirmingDelete === filter.id}
            <button class="danger" onclick={() => remove(filter.id)}
              >{t("filters.reallyDelete")}</button
            >
            <button onclick={() => (confirmingDelete = null)}>{t("filters.keep")}</button>
          {:else}
            <button onclick={() => (confirmingDelete = filter.id)}>{t("filters.delete")}</button>
          {/if}
        </div>

        {#if filter.enabled}
          {#if !orders}
            <p class="hint">{t("orders.looking")}</p>
          {:else if orders.length}
            <ul class="orders">
              {#each orders as order (order.id)}
                {@const gauge = deal(filter.intent, filter.priceLimit, order.platinum)}
                <li>
                  <span class="plat">{order.platinum}<i>p</i></span>
                  <div class="who">
                    <span class="trader">{order.trader}</span>
                    <small>{details(order)}</small>
                  </div>
                  <div class="gauge" title={gauge.label}>
                    <i style:width="{Math.max(gauge.depth, 0.03) * 100}%"></i>
                    <span>{gauge.label}</span>
                  </div>
                  <button onclick={() => copy(order)}>
                    {t(copied === order.id ? "orders.copied" : "orders.copy")}
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="hint">{t("orders.none")}</p>
          {/if}
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  .titlebar {
    position: relative;
    z-index: 10;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 0.9em;
    background: var(--titlebar-bg);
    color: var(--titlebar-text);
    user-select: none;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.7em;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.3em;
  }

  .brand small {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 400;
    letter-spacing: 0.1em;
    opacity: 0.55;
  }

  .led {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--band-light);
    box-shadow: 0 0 6px var(--band-light);
  }

  .led.bad {
    background: var(--bad);
    box-shadow: 0 0 6px var(--bad);
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
    padding: 0;
    font-size: 12px;
    letter-spacing: 0;
    color: inherit;
  }

  .titlebar .controls button:hover {
    color: inherit;
    background: var(--titlebar-hover);
  }

  .titlebar .controls .close:hover {
    background: var(--close-hover);
  }

  .cockpit {
    position: relative;
    height: calc(100vh - 32px);
    display: grid;
    grid-template-columns: minmax(210px, 30%) 1fr;
    grid-template-rows: auto 1fr;
  }

  .cockpit p {
    margin: 0;
  }

  .bus {
    position: relative;
    grid-column: 1 / -1;
    display: flex;
    border-bottom: 1px solid var(--hair);
  }

  .cell {
    flex: none;
    display: flex;
    align-items: center;
    gap: 0.8em;
    padding: 0.45em 0.9em;
    border-right: 1px solid var(--hair);
  }

  .cell.shrink {
    flex: 0 1 auto;
    min-width: 0;
  }

  .cell > :not(.value) {
    flex: none;
  }

  .market {
    position: relative;
  }

  .label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.24em;
    color: var(--soft);
    opacity: 0.7;
  }

  .value {
    min-width: 0;
    overflow: hidden;
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value.ok::before,
  .value.bad::before {
    content: "";
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 0.5em;
    vertical-align: 1px;
    background: var(--accent);
  }

  .value.bad {
    color: var(--bad);
  }

  .value.bad::before {
    background: var(--bad);
  }

  .settings {
    flex: none;
    margin-left: auto;
    border: none;
    border-left: 1px solid var(--hair);
    border-radius: 0;
  }

  .settings.on {
    color: var(--accent);
  }

  .rail {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .rail-head {
    display: flex;
    align-items: center;
    gap: 0.7em;
    padding: 0.8em 0.9em;
    border-bottom: 1px solid var(--hair);
  }

  .count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent);
  }

  .add {
    margin-left: auto;
    padding: 0.2em 0.65em;
    font-size: 15px;
  }

  .slots {
    flex: 1;
    overflow-y: auto;
    scrollbar-gutter: stable;
    list-style: none;
    margin: 0;
    padding: 0.6em;
    display: flex;
    flex-direction: column;
    gap: 0.45em;
  }

  .slot {
    position: relative;
    width: 100%;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.1em 0.7em;
    padding: 0.6em 0.8em 0.6em 0.9em;
    text-align: left;
    text-transform: none;
    letter-spacing: normal;
    font-family: var(--font-ui);
    font-size: 13px;
    color: var(--text);
    background: var(--surface);
    border: none;
    border-left: 3px solid var(--hair);
    clip-path: polygon(0 0, calc(100% - 10px) 0, 100% 10px, 100% 100%, 0 100%);
  }

  .slot:hover:not(:disabled) {
    color: var(--text);
    background: var(--field-bg);
  }

  .slot.current {
    background: var(--field-bg);
    box-shadow: inset 0 0 0 1px var(--hair);
  }

  .slot .code {
    grid-row: 1 / span 3;
    align-self: center;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    color: var(--soft);
  }

  .slot .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .slot .limit,
  .slot .state {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    color: var(--soft);
  }

  .slot .state {
    grid-column: 2;
    height: 14px;
    display: flex;
    align-items: center;
    gap: 0.45em;
  }

  .slot.lock {
    border-left-color: var(--accent);
  }

  .slot.lock .state {
    color: var(--accent);
  }

  .slot.scanning {
    border-left-color: var(--band-light);
  }

  .slot.idle {
    opacity: 0.55;
  }

  /* reticle closes on a lock */
  .slot::before,
  .slot::after {
    content: "";
    position: absolute;
    width: 8px;
    height: 8px;
    border: 0 solid var(--accent);
    opacity: 0;
    transition:
      opacity 160ms ease,
      transform 260ms cubic-bezier(0.2, 0.9, 0.3, 1.3);
  }

  .slot::before {
    left: 6px;
    top: 4px;
    border-top-width: 1.5px;
    border-left-width: 1.5px;
    transform: translate(-6px, -6px);
  }

  .slot::after {
    right: 4px;
    bottom: 4px;
    border-bottom-width: 1.5px;
    border-right-width: 1.5px;
    transform: translate(6px, 6px);
  }

  .slot.lock::before,
  .slot.lock::after {
    opacity: 1;
    transform: none;
  }

  .scan {
    width: 9px;
    height: 9px;
    border: 1.5px solid var(--hair);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: scan 900ms linear infinite;
  }

  @keyframes scan {
    to {
      transform: rotate(360deg);
    }
  }

  .hint {
    padding: 0.8em 0.9em;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.1em;
    color: var(--soft);
  }

  .logs {
    margin: 0.6em;
    background: var(--bg);
  }

  .pane {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 1em 1.2em;
    display: flex;
    flex-direction: column;
    gap: 0.9em;
  }

  .readout header {
    display: flex;
    align-items: center;
    gap: 0.9em;
    padding-bottom: 0.8em;
    border-bottom: 1px solid var(--hair);
  }

  .readout header .code {
    font-family: var(--font-mono);
    font-size: 22px;
    font-weight: 600;
    color: var(--accent);
  }

  .readout.idle header .code {
    color: var(--soft);
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .text strong {
    font-weight: 600;
  }

  .text small {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--soft);
  }

  .tag {
    margin-left: auto;
    padding: 0.2em 0.6em;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.2em;
    color: var(--soft);
    border: 1px solid var(--hair);
  }

  .readout.lock .tag {
    color: var(--on-accent);
    background: var(--accent);
    border-color: var(--accent);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.6em;
    padding: 0.8em 0;
  }

  .switch {
    appearance: none;
    position: relative;
    width: 34px;
    height: 18px;
    margin: 0 0.4em 0 0;
    border: 1px solid var(--hair);
    border-radius: 2px;
    background: var(--field-bg);
    cursor: pointer;
  }

  .switch::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    background: var(--soft);
    opacity: 0.5;
    transition:
      transform 160ms ease,
      background-color 160ms ease;
  }

  .switch:checked {
    border-color: var(--accent);
  }

  .switch:checked::after {
    transform: translateX(16px);
    background: var(--accent);
    opacity: 1;
  }

  .danger,
  .danger:hover:not(:disabled) {
    color: var(--bad);
    border-color: var(--bad);
  }

  .orders {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4em;
  }

  .orders li {
    display: grid;
    grid-template-columns: 4.5em minmax(0, 1fr) minmax(5em, 9em) auto;
    align-items: center;
    gap: 0.9em;
    padding: 0.5em 0.8em;
    background: var(--surface);
  }

  .orders li:first-child {
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .plat {
    font-family: var(--font-mono);
    font-size: 17px;
    font-weight: 600;
    text-align: right;
    color: var(--band-dark);
  }

  .plat i {
    font-style: normal;
    font-size: 11px;
    margin-left: 1px;
    opacity: 0.7;
  }

  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .trader {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .who small {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--soft);
  }

  .gauge {
    position: relative;
    height: 16px;
    background: repeating-linear-gradient(90deg, var(--hair) 0 1px, transparent 1px 10%);
    border: 1px solid var(--hair);
  }

  .gauge i {
    position: absolute;
    inset: 2px auto 2px 2px;
    max-width: calc(100% - 4px);
    background: var(--band-light);
  }

  .orders li:first-child .gauge i {
    background: var(--accent);
  }

  .gauge span {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--soft);
  }

  @media (prefers-reduced-motion: reduce) {
    .scan {
      animation: none;
    }
  }
</style>
