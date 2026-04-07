import { writable } from "svelte/store";
import type { VaultInfo, MediaInfo, SearchResult } from "../api";

export const currentVault = writable<VaultInfo | null>(null);
export const mediaList = writable<MediaInfo[]>([]);
export const currentPage = writable<"search" | "descriptions" | "tags" | "settings">("search");
export const processedCount = writable<number>(0);
export const editMediaId = writable<string | null>(null);
export const isDraggingOut = writable(false);
export const loadingStatus = writable<string>("");
export const previewVolume = writable<number>(0.5);

// Persisted search state so navigating to settings/tags/descriptions and back
// preserves the user's current query, results, and scroll position.
export const searchQueryStore = writable<string>("");
export const searchResultsStore = writable<SearchResult[]>([]);
export const searchHasMoreStore = writable<boolean>(true);
export const mediaHasMoreStore = writable<boolean>(true);
export const searchScrollTop = writable<number>(0);

// Persisted setting: automatic search as you type (default true)
const storedAutoSearch = typeof localStorage !== "undefined" ? localStorage.getItem("autoSearch") : null;
export const autoSearch = writable<boolean>(storedAutoSearch === null ? true : storedAutoSearch === "true");
autoSearch.subscribe((v) => {
  if (typeof localStorage !== "undefined") localStorage.setItem("autoSearch", String(v));
});
