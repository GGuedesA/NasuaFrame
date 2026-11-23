<script lang="ts">
  import {
    addFilter,
    searchItems,
    updateFilter,
    type FilterFields,
    type FilterView,
    type Intent,
    type Item,
    type Platform,
    type Status,
  } from "#lib/api.ts";
  import { t } from "#lib/i18n.svelte.ts";

  type Props = {
    editing: FilterView | null;
    onsaved: () => void;
    oncancel: () => void;
  };
  let { editing, onsaved, oncancel }: Props = $props();

  // parent remounts the form per filter, so reading props once is enough
  function initial() {
    const f = editing;
    return {
      item: f?.item ?? null,
      intent: f?.intent ?? ("buy" as Intent),
      priceLimit: f?.priceLimit ?? 0,
      anyRank: f ? f.rankMin === null : true,
      rankMin: f?.rankMin ?? 0,
      rankMax: f?.rankMax ?? f?.item?.maxRank ?? 0,
      subtype: f?.subtype ?? "",
      platform: f?.platform ?? ("pc" as Platform),
      crossplay: f?.crossplay ?? true,
      minStatus: f?.minStatus ?? ("ingame" as Status),
    };
  }
  let form = $state(initial());
  let error = $state("");
  let saving = $state(false);

  let query = $state("");
  let results = $state<Item[]>([]);
  let searchRun = 0;

  async function search() {
    const run = ++searchRun;
    const q = query.trim();
    const found = q.length < 2 ? [] : await searchItems(q);
    // an older search finishing late must not overwrite a newer one
    if (run === searchRun) results = found;
  }

  function pick(item: Item) {
    form.item = item;
    form.anyRank = true;
    form.rankMin = 0;
    form.rankMax = item.maxRank ?? 0;
    form.subtype = "";
    query = "";
    results = [];
  }

  const isWhole = (n: unknown) => Number.isInteger(n) && (n as number) >= 0;

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const item = form.item;
    if (!item) return void (error = t("form.pickItem"));
    if (!isWhole(form.priceLimit)) return void (error = t("form.wholePrice"));
    const ranked = item.maxRank !== null && !form.anyRank;
    if (ranked && !(isWhole(form.rankMin) && isWhole(form.rankMax)))
      return void (error = t("form.wholeRanks"));

    const fields: FilterFields = {
      itemId: item.id,
      intent: form.intent,
      priceLimit: form.priceLimit,
      platform: form.platform,
      crossplay: form.crossplay,
      rankMin: ranked ? form.rankMin : null,
      rankMax: ranked ? form.rankMax : null,
      subtype: form.subtype || null,
      minStatus: form.minStatus,
    };
    saving = true;
    error = "";
    try {
      if (editing) {
        await updateFilter({ ...editing, ...fields });
      } else {
        await addFilter(fields);
      }
      onsaved();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<form class="card" onsubmit={save}>
  <h2>{t(editing ? "form.edit" : "form.new")}</h2>

  <div class="field">
    <label for="item-search">{t("form.item")}</label>
    {#if form.item}
      <div class="picked">
        <strong>{form.item.name}</strong>
        <button type="button" onclick={() => (form.item = null)}>{t("form.change")}</button>
      </div>
    {:else}
      <input
        id="item-search"
        placeholder={t("form.search")}
        autocomplete="off"
        bind:value={query}
        oninput={search}
      />
      {#if results.length}
        <ul class="results">
          {#each results as item (item.id)}
            <li><button type="button" onclick={() => pick(item)}>{item.name}</button></li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>

  <div class="field">
    <span>{t("form.intent")}</span>
    <div class="row">
      <label><input type="radio" value="buy" bind:group={form.intent} /> {t("form.buy")}</label>
      <label><input type="radio" value="sell" bind:group={form.intent} /> {t("form.sell")}</label>
    </div>
  </div>

  <div class="field">
    <label for="price">{t(form.intent === "buy" ? "form.maxPrice" : "form.minPrice")}</label>
    <input id="price" type="number" min="0" step="1" bind:value={form.priceLimit} />
  </div>

  {#if form.item && form.item.maxRank !== null}
    <div class="field">
      <span>{t("form.rank")}</span>
      <label><input type="checkbox" bind:checked={form.anyRank} /> {t("form.anyRank")}</label>
      {#if !form.anyRank}
        <div class="row">
          <label>
            {t("form.rankFrom")}
            <input type="number" min="0" max={form.item.maxRank} bind:value={form.rankMin} />
          </label>
          <label>
            {t("form.rankTo")}
            <input type="number" min="0" max={form.item.maxRank} bind:value={form.rankMax} />
          </label>
        </div>
      {/if}
    </div>
  {/if}

  {#if form.item && form.item.subtypes.length}
    <div class="field">
      <label for="subtype">{t("form.subtype")}</label>
      <select id="subtype" bind:value={form.subtype}>
        <option value="">{t("form.anySubtype")}</option>
        {#each form.item.subtypes as subtype (subtype)}
          <option value={subtype}>{subtype}</option>
        {/each}
      </select>
    </div>
  {/if}

  <div class="field">
    <label for="platform">{t("form.platform")}</label>
    <div class="row">
      <select id="platform" bind:value={form.platform}>
        <option value="pc">{t("platform.pc")}</option>
        <option value="ps4">{t("platform.ps4")}</option>
        <option value="xbox">{t("platform.xbox")}</option>
        <option value="switch">{t("platform.switch")}</option>
        <option value="mobile">{t("platform.mobile")}</option>
      </select>
      <label><input type="checkbox" bind:checked={form.crossplay} /> {t("form.crossplay")}</label>
    </div>
  </div>

  <div class="field">
    <label for="status">{t("form.minStatus")}</label>
    <select id="status" bind:value={form.minStatus}>
      <option value="ingame">{t("form.ingame")}</option>
      <option value="online">{t("form.online")}</option>
      <option value="offline">{t("form.offline")}</option>
    </select>
  </div>

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <div class="row">
    <button type="submit" disabled={saving}>{t("save")}</button>
    <button type="button" onclick={oncancel}>{t("cancel")}</button>
  </div>
</form>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3em;
    margin-bottom: 0.9em;
    text-align: left;
  }

  .picked {
    display: flex;
    align-items: center;
    gap: 0.6em;
  }

  .results {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 14em;
    overflow-y: auto;
  }

  .results button {
    width: 100%;
    text-align: left;
    border-color: transparent;
    font-family: inherit;
    font-size: 1em;
    letter-spacing: normal;
    text-transform: none;
    color: var(--text);
  }
</style>
