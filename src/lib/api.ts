import { invoke } from "@tauri-apps/api/core";

export interface VaultInfo {
  path: string;
  media_count: number;
}

export interface MediaInfo {
  id: string;
  extension: string;
  media_type: string;
  codec: string | null;
  has_description: boolean;
  description: string | null;
}

export interface SearchResult {
  media: MediaInfo;
  score: number;
}

export interface DescriptionPageData {
  media: MediaInfo;
  description: string | null;
  current_index: number;
  total_count: number;
}

export async function getLoadingStatus(): Promise<string> {
  return invoke("get_loading_status");
}

export async function createVault(path: string): Promise<VaultInfo> {
  return invoke("create_vault", { path });
}

export async function openVault(path: string): Promise<VaultInfo> {
  return invoke("open_vault", { path });
}

export async function closeVault(): Promise<void> {
  return invoke("close_vault");
}

export async function getLastVault(): Promise<string | null> {
  return invoke("get_last_vault");
}

export async function getZoomLevel(): Promise<number | null> {
  return invoke("get_zoom_level");
}

export async function setZoomLevel(level: number): Promise<void> {
  return invoke("set_zoom_level", { level });
}

export async function refreshVault(): Promise<void> {
  return invoke("refresh_vault");
}

export async function importMedia(filePaths: string[]): Promise<void> {
  return invoke("import_media", { filePaths });
}

export async function getMediaList(
  offset: number,
  limit: number
): Promise<MediaInfo[]> {
  return invoke("get_media_list", { offset, limit });
}

export async function getMediaThumbnail(mediaId: string): Promise<string> {
  return invoke("get_media_thumbnail", { mediaId });
}

export async function getThumbnailPath(mediaId: string): Promise<string> {
  return invoke("get_thumbnail_path", { mediaId });
}

export async function getPreviewData(mediaId: string): Promise<string | null> {
  return invoke("get_preview_data", { mediaId });
}

export async function getPreviewFilePath(mediaId: string): Promise<string | null> {
  return invoke("get_preview_file_path", { mediaId });
}

export function toMediaUrl(path: string): string {
  const forwardSlash = path.replace(/\\/g, "/");
  const normalized = forwardSlash.startsWith("/") ? forwardSlash : "/" + forwardSlash;
  return "http://media.localhost" + normalized;
}

export async function getMediaPath(mediaId: string): Promise<string> {
  return invoke("get_media_path", { mediaId });
}

export async function deleteMedia(mediaId: string): Promise<void> {
  return invoke("delete_media", { mediaId });
}

export async function revealMedia(mediaId: string): Promise<void> {
  return invoke("reveal_media", { mediaId });
}

export async function copyMediaFile(mediaId: string): Promise<void> {
  return invoke("copy_media_file", { mediaId });
}

export async function copyMediaPath(mediaId: string): Promise<void> {
  return invoke("copy_media_path", { mediaId });
}

export async function searchMedia(
  query: string,
  limit: number = 50
): Promise<SearchResult[]> {
  return invoke("search_media", { query, limit });
}

export async function getDescription(
  mediaId: string
): Promise<string | null> {
  return invoke("get_description", { mediaId });
}

export async function setDescription(
  mediaId: string,
  description: string
): Promise<void> {
  return invoke("set_description", { mediaId, description });
}

export async function getMediaForDescription(
  index: number,
  filterMissing: boolean
): Promise<DescriptionPageData | null> {
  return invoke("get_media_for_description", { index, filterMissing });
}

export async function getMediaIndex(
  mediaId: string,
  filterMissing: boolean
): Promise<number | null> {
  return invoke("get_media_index", { mediaId, filterMissing });
}
