<script lang="ts">
  const art = import.meta.glob<string>("./assets/ornaments/*.svg", {
    query: "?raw",
    import: "default",
    eager: true,
  });

  let {
    name,
  }: { name: "symbols" | "topdiv" | "topright" | "sidediv" | "sidelow" | "botleft" | "botright" } =
    $props();
</script>

<div class="ornament {name}" aria-hidden="true">
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html art[`./assets/ornaments/${name}.svg`]}
</div>

<style>
  .ornament {
    position: absolute;
    line-height: 0;
    pointer-events: none;
  }

  .ornament :global {
    svg {
      display: block;
      width: 100%;
      height: auto;
      overflow: visible;
    }

    .traces polyline,
    .traces circle {
      fill: none;
      stroke: var(--orn-trace);
      stroke-linejoin: miter;
      stroke-linecap: square;
    }

    .stems path {
      fill: none;
      stroke: var(--orn-stem);
      stroke-width: 1;
      stroke-linecap: round;
      stroke-linejoin: round;
    }

    .stems .twig {
      stroke-width: 0.6;
    }

    .sprig-stem {
      fill: none;
      stroke: var(--orn-stem);
      stroke-width: 0.45;
    }

    .tone-a {
      fill: var(--orn-leaf-a);
    }

    .tone-b {
      fill: var(--orn-leaf-b);
    }

    .tone-c {
      fill: var(--orn-leaf-c);
    }

    .vein {
      fill: none;
      stroke: var(--orn-vein);
      stroke-opacity: 0.35;
      stroke-width: 0.3;
    }

    .petal {
      fill: var(--orn-flower);
    }

    .center {
      fill: var(--orn-flower-center);
    }
  }

  .symbols {
    width: 0;
    height: 0;
    overflow: hidden;
  }

  .topdiv {
    top: 0;
    left: -12px;
    width: 21.8px;
  }

  .topright {
    grid-column: 1 / -1;
    grid-row: 1 / -1;
    top: 0;
    right: 0;
    width: 221.7px;
  }

  /* lines keep going when the window is taller */
  .sidediv {
    grid-column: 1 / 2;
    grid-row: 2;
    top: 0;
    bottom: 0;
    left: calc(100% - 25px);
    width: 48.6px;
    background:
      linear-gradient(var(--orn-line), var(--orn-line)) 21px 264px / 1px 100% no-repeat,
      linear-gradient(var(--orn-line), var(--orn-line)) 26px 0 / 1px 100% no-repeat;
    --orn-line: color-mix(in srgb, var(--orn-trace) 90%, transparent);
  }

  .sidelow {
    grid-column: 1 / 2;
    grid-row: 2;
    bottom: 0;
    left: calc(100% - 25px);
    width: 48.6px;
  }

  .botleft {
    grid-column: 1 / 2;
    grid-row: 2;
    left: 0;
    bottom: 0;
    width: 108.9px;
  }

  .botright {
    grid-column: 2;
    grid-row: 2;
    right: 0;
    bottom: 0;
    width: 280px;
  }
</style>
