<script lang="ts">
  type Props = { grown?: boolean; delay?: number };
  let { grown = true, delay = 0 }: Props = $props();

  function frond(len: number, bend: number, angle: number) {
    const leaves: { d: string; transform: string }[] = [];
    let stem = "M0 0";
    const count = 16;
    for (let k = 1; k <= count; k++) {
      const s = k / count;
      const x = bend * s * s * len;
      const y = -s * len;
      stem += ` L${x.toFixed(1)} ${y.toFixed(1)}`;
      if (k === count) break;
      const h = len * 0.22 * (1 - s * 0.75);
      const w = h * 0.2;
      const lean = (Math.atan(2 * bend * s) * 180) / Math.PI;
      for (const side of [-1, 1]) {
        leaves.push({
          d: `M0 0 Q${w.toFixed(1)} ${(-h / 2).toFixed(1)} 0 ${(-h).toFixed(1)} Q${(-w).toFixed(1)} ${(-h / 2).toFixed(1)} 0 0`,
          transform: `translate(${x.toFixed(1)} ${y.toFixed(1)}) rotate(${(lean + side * 58).toFixed(1)})`,
        });
      }
    }
    return { stem, leaves, angle };
  }

  const fronds = [frond(380, 0.22, -38), frond(330, 0.28, 2), frond(250, -0.25, 42)];
</script>

<svg viewBox="-300 -400 600 410" class:grown aria-hidden="true">
  <g class="sway" style:animation-delay="{delay}ms">
    {#each fronds as f, i (i)}
      <g transform="rotate({f.angle})">
        <path d={f.stem} pathLength="1" style:transition-delay="{i * 200}ms" />
        {#each f.leaves as leaf, j (j)}
          <path
            {...leaf}
            pathLength="1"
            style:transition-delay="{i * 200 + 300 + Math.floor(j / 2) * 70}ms"
          />
        {/each}
      </g>
    {/each}
  </g>
</svg>

<style>
  svg {
    display: block;
    width: 100%;
  }

  path {
    fill: none;
    stroke: var(--leaf);
    stroke-width: 1.2;
    vector-effect: non-scaling-stroke;
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    transition: stroke-dashoffset 900ms ease-out;
  }

  .grown path {
    stroke-dashoffset: 0;
  }

  .sway {
    animation: sway 9s ease-in-out infinite alternate;
  }

  @keyframes sway {
    from {
      transform: rotate(-1.5deg);
    }
    to {
      transform: rotate(1.5deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sway {
      animation: none;
    }

    path {
      transition: none;
    }
  }
</style>
