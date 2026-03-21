import { writable } from "svelte/store";
import type { VaultInfo, MediaInfo } from "../api";

export const currentVault = writable<VaultInfo | null>(null);
export const mediaList = writable<MediaInfo[]>([]);
export const currentPage = writable<"search" | "descriptions">("search");
export const editMediaId = writable<string | null>(null);
export const isDraggingOut = writable(false);
export const loadingStatus = writable<string>("");
export const previewVolume = writable<number>(0.5);
