# Changelog

All notable changes to this project are documented here. Format: [Keep a Changelog](https://keepachangelog.com), versions follow [SemVer](https://semver.org).

## [Unreleased]

- Every matching order notifies now, the per-filter cooldown was hiding other sellers' orders.

## [0.1.0] - 2026-11-16

First version for test.

- Filters for warframe.market orders: item, buy or sell, price limit, rank, subtype, platform, crossplay and the trader's minimum status.
- Live matches from the warframe.market order stream, with the whisper ready to copy.
- Desktop notification for each match while the window isn't focused.
- Runs in the tray, closing the window keeps it watching. Only one copy runs at a time.
- Reconnects on its own after a dropped connection.
- Item list downloaded on start and kept for offline use.
- Log file, "Open log folder" sends you to it.

## [0.0.2] - 2026-11-10

- Test release to verify automatic updates from 0.0.1.

## [0.0.1] - 2026-11-10

- Project skeleton: Tauri 2 + SvelteKit app, Rust workspace, CI and release workflows.
- Automatic updates through GitHub Releases.
