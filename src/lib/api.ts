import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// mirrors of the rust structs in src-tauri/src/commands.rs and watcher.rs

export type Intent = "buy" | "sell";
export type Platform = "pc" | "ps4" | "xbox" | "switch" | "mobile";
export type Status = "offline" | "online" | "ingame";

export type Item = {
  id: string;
  slug: string;
  name: string;
  maxRank: number | null;
  subtypes: string[];
};

export type FilterFields = {
  itemId: string;
  intent: Intent;
  priceLimit: number;
  platform?: Platform;
  crossplay?: boolean;
  rankMin: number | null;
  rankMax: number | null;
  subtype: string | null;
  minStatus?: Status;
};

export type FilterView = FilterFields & {
  id: number;
  enabled: boolean;
  cooldownSecs: number;
  item: Item | null;
};

export type CatalogStatus = { version: string | null; error: string | null };

export type Alert = {
  filterId: number;
  itemName: string;
  platinum: number;
  trader: string;
  whisper: string;
};

export const listFilters = () => invoke<FilterView[]>("list_filters");
export const addFilter = (filter: FilterFields) => invoke<number>("add_filter", { filter });
export const updateFilter = (filter: FilterView) => invoke<void>("update_filter", { filter });
export const deleteFilter = (id: number) => invoke<void>("delete_filter", { id });
export const searchItems = (query: string) => invoke<Item[]>("search_items", { query });
export const refreshCatalog = () => invoke<CatalogStatus>("refresh_catalog");
export const openLogFolder = () => invoke<void>("open_log_folder");

export const onAlert = (handler: (alert: Alert) => void): Promise<UnlistenFn> =>
  listen<Alert>("alert", (event) => handler(event.payload));
/** problem null means the feed is connected */
export type FeedStatus = { feed: string; problem: string | null };

export const onFeedStatus = (handler: (status: FeedStatus) => void): Promise<UnlistenFn> =>
  listen<FeedStatus>("feed-status", (event) => handler(event.payload));
