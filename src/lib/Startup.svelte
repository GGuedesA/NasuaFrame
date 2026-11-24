<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import Fern from "#lib/Fern.svelte";
  import type { CatalogStatus } from "#lib/api.ts";
  import { t, type Key } from "#lib/i18n.svelte.ts";

  type Props = {
    // null while loading
    settings: boolean | null;
    catalog: CatalogStatus | null;
    enabledFilters: number | null;
    feedProblem: boolean;
    ondone: () => void;
  };
  let { settings, catalog, enabledFilters, feedProblem, ondone }: Props = $props();

  type Step = {
    word: Key;
    title: Key;
    ready: () => boolean;
    result: () => [Key, boolean, Record<string, number>?];
  };

  const steps: Step[] = [
    {
      word: "startup.files",
      title: "startup.filesTitle",
      ready: () => settings !== null,
      result: () => (settings ? ["startup.filesOk", true] : ["startup.filesFailed", false]),
    },
    {
      word: "startup.catalog",
      title: "startup.catalogTitle",
      ready: () => catalog !== null,
      result: () =>
        !catalog?.version
          ? ["startup.catalogFailed", false]
          : catalog.error
            ? ["startup.catalogOffline", false]
            : ["startup.catalogReady", true],
    },
    {
      word: "startup.market",
      title: "startup.marketTitle",
      ready: () => enabledFilters !== null,
      result: () =>
        feedProblem
          ? ["startup.feedProblem", false]
          : enabledFilters
            ? ["startup.watching", true, { count: enabledFilters }]
            : ["startup.noFilters", true],
    },
  ];

  const BANDS = 12;
  const perStep = BANDS / steps.length;

  let phase = $state<"black" | "plate" | "out">("black");
  let nameIn = $state(false);
  let booted = $state(0);
  let curl = $state(0);
  let spinning = $state(false);
  let seam = $state(false);
  let index = $state(0);
  let result = $state<[string, boolean] | null>(null);

  let done = $derived(index >= steps.length);
  let lit = $derived(Math.round((index + (result ? 1 : 0)) * perStep));

  let finished = false;
  function finish() {
    if (finished) return;
    finished = true;
    phase = "out";
    setTimeout(ondone, 700);
  }

  const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

  async function until(ready: () => boolean, max: number) {
    const end = Date.now() + max;
    while (!finished && !ready() && Date.now() < end) await sleep(50);
  }

  function curlUp(ms: number) {
    return new Promise<void>((resolve) => {
      const start = performance.now();
      const frame = (now: number) => {
        const p = Math.min((now - start) / ms, 1);
        curl = p < 0.5 ? 4 * p * p * p : 1 - (-2 * p + 2) ** 3 / 2;
        if (p < 1) requestAnimationFrame(frame);
        else resolve();
      };
      requestAnimationFrame(frame);
    });
  }

  async function intro() {
    await sleep(300);
    nameIn = true;
    await sleep(300);
    for (let k = 1; k <= BANDS; k++) {
      if (finished) return;
      booted = k;
      await sleep(80);
    }
    await sleep(350);
    nameIn = false;
    await curlUp(1000);
    spinning = true;
    seam = true;
    await sleep(300);
    if (finished) return;
    phase = "plate";
    await sleep(900);
  }

  async function run() {
    // windows animation effects off, the intro would only flash its frames
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
      curl = 1;
      phase = "plate";
    } else {
      await intro();
    }
    for (let i = 0; i < steps.length; i++) {
      if (finished) return;
      index = i;
      result = null;
      // first catalog download can be slow
      await Promise.all([sleep(700), until(steps[i].ready, 10000)]);
      if (finished) return;
      const [key, ok, values] = steps[i].ready()
        ? steps[i].result()
        : ["startup.slow" as Key, false];
      result = [t(key, values), ok];
      await sleep(550);
    }
    if (finished) return;
    index = steps.length;
    result = null;
    await sleep(900);
    finish();
  }

  onMount(() => {
    document.documentElement.classList.remove("booting");
    // window starts hidden to avoid the white blink
    requestAnimationFrame(() => getCurrentWindow().show());
    run();
  });

  // u = 0 straight bar, u = 1 the ring
  function band(i: number, u: number) {
    const len = 360 + (2 * Math.PI * 160 - 360) * u;
    const piece = len / BANDS;
    const gap = piece / 15;
    const from = len / 2 - i * piece - gap;
    const to = len / 2 - (i + 1) * piece + gap;
    if (u < 0.001) return `M${200 + from} 200 L${200 + to} 200`;
    const rho = len / (2 * Math.PI * u);
    const cy = 200 + 160 * u - rho;
    const point = (s: number) =>
      `${(200 + rho * Math.sin(s / rho)).toFixed(2)} ${(cy + rho * Math.cos(s / rho)).toFixed(2)}`;
    return `M${point(from)} A${rho.toFixed(2)} ${rho.toFixed(2)} 0 0 1 ${point(to)}`;
  }

  let bands = $derived(Array.from({ length: BANDS }, (_, i) => band(i, curl)));

  const ticks = Array.from({ length: 60 }, (_, i) => {
    const rad = ((i * 6 - 90) * Math.PI) / 180;
    const outer = i % 5 === 0 ? 142 : 136;
    return {
      x1: 200 + 128 * Math.cos(rad),
      y1: 200 + 128 * Math.sin(rad),
      x2: 200 + outer * Math.cos(rad),
      y2: 200 + outer * Math.sin(rad),
    };
  });

  const pad = (n: number) => String(n).padStart(2, "0");
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && finish()} />

<div class="startup" class:out={phase === "out"} data-tauri-drag-region>
  {#each ["br", "tl"] as corner (corner)}
    <div class="fern {corner}">
      <Fern grown={phase === "plate" || phase === "out"} delay={corner === "tl" ? -4000 : 0} />
    </div>
  {/each}

  <i class="corner tl"></i><i class="corner tr"></i><i class="corner bl"></i><i class="corner br"
  ></i>

  <div class="center" class:in={phase === "plate" || phase === "out"} role="status">
    <div class="ring">
      <svg viewBox="0 0 400 400" aria-hidden="true" class:boot={phase === "black"}>
        <g class="spin" class:spinning>
          {#each bands as d, i (i)}
            <!-- band 0 is the bar's right end -->
            <path {d} class="track" class:dark={i % 2 === 0} class:on={BANDS - 1 - i < booted} />

            <path
              {d}
              pathLength="1"
              class="fill"
              class:dark={i % 2 === 0}
              class:lit={i < lit}
              style:transition-delay="{(i % perStep) * 140}ms"
            />
          {/each}
        </g>
        <g class="spin-back">
          <circle cx="200" cy="200" r="128" />
          {#each ticks as tick, i (i)}
            <line {...tick} />
          {/each}
        </g>
      </svg>
      <div class="word">{t(done ? "startup.ready" : steps[index].word)}</div>
      <div class="name" class:in={nameIn}>NASUAFRAME</div>
    </div>
    <div class="title">{t(done ? "startup.readyTitle" : steps[index].title)}</div>
    <div class="result" class:bad={result && !result[1]}>{result?.[0] ?? ""}</div>
  </div>

  <div class="counter">
    [ {pad(Math.min(index + 1, steps.length + 1))} / {pad(steps.length + 1)} ]
  </div>
  <button class="skip" onclick={finish}>{t("startup.skip")} &#x25B8;</button>

  <div class="black" class:open={phase !== "black"}>
    <div class="shutter a"></div>
    <div class="shutter b"></div>
    <svg class="seam" class:lit={seam} viewBox="0 0 100 100" preserveAspectRatio="none">
      <line x1="0" y1="60" x2="100" y2="40" />
    </svg>
  </div>
</div>

<style>
  .startup {
    position: fixed;
    inset: 0;
    z-index: 1000;
    overflow: hidden;
    background: var(--plate);
    color: var(--ink);
    font-family: var(--font-display);
    line-height: normal;
    user-select: none;
    transition:
      opacity 700ms ease,
      transform 700ms ease,
      filter 700ms ease;
  }

  .startup.out {
    opacity: 0;
    transform: scale(1.04);
    filter: blur(6px);
  }

  .center,
  .black,
  .wash,
  .corner {
    pointer-events: none;
  }

  .corner {
    position: absolute;
    width: 18px;
    height: 18px;
    border: 0 solid var(--hair);
  }

  .tl {
    top: 20px;
    left: 20px;
    border-top-width: 2px;
    border-left-width: 2px;
  }

  .tr {
    top: 20px;
    right: 20px;
    border-top-width: 2px;
    border-right-width: 2px;
  }

  .bl {
    bottom: 20px;
    left: 20px;
    border-bottom-width: 2px;
    border-left-width: 2px;
  }

  .br {
    bottom: 20px;
    right: 20px;
    border-bottom-width: 2px;
    border-right-width: 2px;
  }

  .center {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  /* ring stays above the shutters */
  .word,
  .title,
  .result,
  .spin-back {
    opacity: 0;
    transition: opacity 700ms ease 300ms;
  }

  .center.in :is(.word, .title, .result, .spin-back) {
    opacity: 1;
  }

  .ring {
    position: relative;
    z-index: 2;
    width: min(62vmin, 400px);
    aspect-ratio: 1;
  }

  .ring svg {
    width: 100%;
    height: 100%;
  }

  .spin {
    transform-origin: 200px 200px;
  }

  .spin.spinning {
    animation: spin 10s linear infinite;
  }

  .spin-back {
    transform-origin: 200px 200px;
    animation: spin 24s linear infinite reverse;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .track,
  .fill {
    fill: none;
    stroke-width: 22;
  }

  .track {
    stroke: var(--hair);
    transition:
      stroke 800ms ease,
      opacity 120ms linear;
  }

  .boot .track {
    stroke: var(--band-light);
  }

  .boot .track.dark {
    stroke: var(--accent);
  }

  .boot .track:not(.on) {
    opacity: 0;
  }

  .fill {
    stroke: var(--band-light);
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    transition: stroke-dashoffset 140ms linear;
  }

  .fill.dark {
    stroke: var(--band-dark);
  }

  .fill.lit {
    stroke-dashoffset: 0;
  }

  .fern {
    position: absolute;
    width: min(75vmin, 560px);
    pointer-events: none;
  }

  .fern.br {
    right: -12vmin;
    bottom: -2vmin;
    transform: scaleX(-1);
  }

  .fern.tl {
    left: -12vmin;
    top: -2vmin;
    transform: scaleY(-1);
  }

  .spin-back circle {
    fill: none;
    stroke: var(--hair);
  }

  .spin-back line {
    stroke: var(--soft);
    stroke-width: 1.2;
  }

  .word {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: clamp(16px, 4.4vmin, 30px);
    font-weight: 700;
    letter-spacing: 0.3em;
    text-indent: 0.3em;
  }

  .title {
    margin-top: 18px;
    font-size: clamp(16px, 3.2vmin, 24px);
    letter-spacing: 0.16em;
    text-indent: 0.16em;
    text-transform: uppercase;
  }

  .result {
    margin-top: 10px;
    min-height: 1.4em;
    font-family: var(--font-mono);
    font-size: 12px;
    letter-spacing: 0.3em;
    text-indent: 0.3em;
    text-transform: uppercase;
    color: var(--soft);
  }

  .result.bad {
    color: var(--bad);
  }

  .counter {
    position: absolute;
    left: 46px;
    bottom: 40px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.24em;
    color: var(--soft);
  }

  .skip {
    position: absolute;
    top: 36px;
    right: 46px;
    border: none;
    background: none;
    box-shadow: none;
    padding: 4px 0;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.24em;
    color: var(--soft);
    cursor: pointer;
  }

  .skip:hover {
    color: var(--accent);
    background: none;
  }

  .black {
    position: absolute;
    inset: 0;
    z-index: 1;
  }

  .shutter {
    position: absolute;
    inset: 0;
    background: var(--night);
    transition:
      transform 1300ms cubic-bezier(0.45, 0, 0.2, 1),
      opacity 1100ms ease 150ms;
  }

  .shutter.a {
    clip-path: polygon(0 0, 100% 0, 100% calc(40% + 1px), 0 calc(60% + 1px));
  }

  .shutter.b {
    clip-path: polygon(0 60%, 100% 40%, 100% 100%, 0 100%);
  }

  .open .shutter {
    opacity: 0;
  }

  .open .shutter.a {
    transform: translateY(-80%);
  }

  .open .shutter.b {
    transform: translateY(80%);
  }

  .seam {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    transition: opacity 200ms linear;
  }

  .seam line {
    stroke: var(--band-light);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }

  .seam.lit {
    opacity: 1;
  }

  .open .seam {
    opacity: 0;
    transition-duration: 400ms;
  }

  .name {
    position: absolute;
    top: 60%;
    left: -50%;
    right: -50%;
    text-align: center;
    color: var(--night-text);
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 0.4em;
    text-indent: 0.4em;
    opacity: 0;
    transition: opacity 400ms ease;
  }

  .name.in {
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .startup,
    .startup * {
      animation: none !important;
      transition-duration: 1ms !important;
    }
  }
</style>
