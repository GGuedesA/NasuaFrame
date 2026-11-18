# Changelog

All notable changes to this project are documented here. Format: [Keep a Changelog](https://keepachangelog.com), versions follow [SemVer](https://semver.org).

## [Unreleased]

- Every matching order notifies now, the per-filter cooldown was hiding other sellers' orders.
- Each filter shows its best 5 orders right now, cheapest first when buying and highest first when selling. Orders already listed show up when the filter starts, and the list refreshes every 2 minutes so sold orders and offline traders drop out.
- The separate Matches list is gone, the orders are under each filter.
- Settings screen: language (English or Português) and whether closing the window keeps the app in the tray or quits it.
- Item names follow the chosen language (English when an item has no translation). Whispers stay in English.
- The app is available in Portuguese, including the tray menu and notifications.

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
