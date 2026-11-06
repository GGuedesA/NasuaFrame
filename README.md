# NasuaFrame

A lightweight, open-source desktop companion for Warframe players, for Windows and Linux.

**Status:** early development. Nothing to install yet.

## Why this project exists

NasuaFrame is first of all a learning project. I'm using it to learn Rust and to practise
the fundamentals of building real software end to end: a clean project structure, tests,
linting, CI/CD, signed releases with automatic updates, cross-platform support and
translations.

The goal is to do these things properly, even where a quick hack would work, and to
end up with a tool that I and others actually use. Expect the code to
change as I learn. Feedback, suggestions and pull requests are welcome.

## Planned features

- Real-time warframe.market order alerts with per-item filters
- One-click copy of the in-game whisper message
- Automatic updates

## Development

Requirements: [Rust](https://rustup.rs) (stable), Node LTS, pnpm, and the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```sh
pnpm install
pnpm tauri dev
```

Release installers are built and signed by GitHub Actions. To build an installer locally
without the updater signing key, turn off the updater artifacts for that build:

```sh
pnpm tauri build -c '{"bundle":{"createUpdaterArtifacts":false}}'
```

## License

[Apache-2.0](LICENSE)

## Disclaimer

NasuaFrame is not affiliated with or endorsed by Digital Extremes or warframe.market.
Warframe is a trademark of Digital Extremes.
