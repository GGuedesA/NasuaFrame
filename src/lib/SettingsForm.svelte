<script lang="ts">
  import { getSettings, saveSettings, type Settings } from "#lib/api.ts";
  import { setLanguage, t } from "#lib/i18n.svelte.ts";

  type Props = { onsaved: () => void; oncancel: () => void };
  let { onsaved, oncancel }: Props = $props();

  // only languages with a ui translation, the rest would leave the app in english
  const languages = [
    { code: "en", name: "English" },
    { code: "pt", name: "Português" },
  ];

  let settings = $state<Settings | null>(null);
  let error = $state("");
  let saving = $state(false);

  getSettings()
    .then((s) => (settings = s))
    .catch((e) => (error = String(e)));

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!settings) return;
    saving = true;
    error = "";
    try {
      await saveSettings(settings);
      setLanguage(settings.language);
      onsaved();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<form class="card" onsubmit={save}>
  <h2>{t("settings.title")}</h2>

  {#if settings}
    <div class="field">
      <label for="language">{t("settings.language")}</label>
      <select id="language" bind:value={settings.language}>
        {#each languages as language (language.code)}
          <option value={language.code}>{language.name}</option>
        {/each}
      </select>
    </div>

    <div class="field">
      <label for="close">{t("settings.close")}</label>
      <select id="close" bind:value={settings.closeToTray}>
        <option value={true}>{t("settings.closeToTray")}</option>
        <option value={false}>{t("settings.closeQuits")}</option>
      </select>
    </div>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <div class="row">
    <button type="submit" disabled={!settings || saving}>{t("save")}</button>
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
</style>
