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
      cooldownSecs: f?.cooldownSecs ?? 60,
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
    if (!item) return void (error = "Pick an item first");
    if (!isWhole(form.priceLimit)) return void (error = "Price must be a whole number");
    const ranked = item.maxRank !== null && !form.anyRank;
    if (ranked && !(isWhole(form.rankMin) && isWhole(form.rankMax)))
      return void (error = "Ranks must be whole numbers");
    if (!isWhole(form.cooldownSecs)) return void (error = "Seconds must be a whole number");

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
        await updateFilter({ ...editing, ...fields, cooldownSecs: form.cooldownSecs });
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
  <h2>{editing ? "Edit filter" : "New filter"}</h2>

  <div class="field">
    <label for="item-search">Item</label>
    {#if form.item}
      <div class="picked">
        <strong>{form.item.name}</strong>
        <button type="button" onclick={() => (form.item = null)}>Change</button>
      </div>
    {:else}
      <input
        id="item-search"
        placeholder="Type at least 2 letters"
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
    <span>I want to</span>
    <div class="row">
      <label><input type="radio" value="buy" bind:group={form.intent} /> Buy</label>
      <label><input type="radio" value="sell" bind:group={form.intent} /> Sell</label>
    </div>
  </div>

  <div class="field">
    <label for="price">{form.intent === "buy" ? "Max price" : "Min price"} (platinum)</label>
    <input id="price" type="number" min="0" step="1" bind:value={form.priceLimit} />
  </div>

  {#if form.item && form.item.maxRank !== null}
    <div class="field">
      <span>Rank</span>
      <label><input type="checkbox" bind:checked={form.anyRank} /> Any rank</label>
      {#if !form.anyRank}
        <div class="row">
          <label>
            From
            <input type="number" min="0" max={form.item.maxRank} bind:value={form.rankMin} />
          </label>
          <label>
            to
            <input type="number" min="0" max={form.item.maxRank} bind:value={form.rankMax} />
          </label>
        </div>
      {/if}
    </div>
  {/if}

  {#if form.item && form.item.subtypes.length}
    <div class="field">
      <label for="subtype">Subtype</label>
      <select id="subtype" bind:value={form.subtype}>
        <option value="">Any</option>
        {#each form.item.subtypes as subtype (subtype)}
          <option value={subtype}>{subtype}</option>
        {/each}
      </select>
    </div>
  {/if}

  <div class="field">
    <label for="platform">Platform</label>
    <div class="row">
      <select id="platform" bind:value={form.platform}>
        <option value="pc">PC</option>
        <option value="ps4">PlayStation</option>
        <option value="xbox">Xbox</option>
        <option value="switch">Switch</option>
        <option value="mobile">Mobile</option>
      </select>
      <label><input type="checkbox" bind:checked={form.crossplay} /> Crossplay</label>
    </div>
  </div>

  <div class="field">
    <label for="status">Trader at least</label>
    <select id="status" bind:value={form.minStatus}>
      <option value="ingame">In game</option>
      <option value="online">Online</option>
      <option value="offline">Anyone, even offline</option>
    </select>
  </div>

  {#if editing}
    <div class="field">
      <label for="cooldown">Seconds between alerts</label>
      <input id="cooldown" type="number" min="0" step="1" bind:value={form.cooldownSecs} />
    </div>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <div class="row">
    <button type="submit" disabled={saving}>Save</button>
    <button type="button" onclick={oncancel}>Cancel</button>
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
    box-shadow: none;
  }
</style>
