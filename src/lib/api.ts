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
  duration: number | null;
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

export async function getBuildInfo(): Promise<[string, string]> {
  return invoke("get_build_info");
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

export async function deepRefresh(): Promise<void> {
  return invoke("deep_refresh");
}

export async function regenerateAllPreviews(): Promise<void> {
  return invoke("regenerate_all_previews");
}

export async function pauseProcessing(): Promise<void> {
  return invoke("pause_processing");
}

export async function resumeProcessing(): Promise<void> {
  return invoke("resume_processing");
}

export async function getProcessedCount(): Promise<number> {
  return invoke("get_processed_count");
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
  offset: number = 0,
  limit: number = 200,
): Promise<SearchResult[]> {
  return invoke("search_media", { query, offset, limit });
}

export async function getMediaForDescription(
  index: number,
  filterMissingDesc: boolean,
  filterMissingTags: boolean,
): Promise<DescriptionPageData | null> {
  return invoke("get_media_for_description", { index, filterMissingDesc, filterMissingTags });
}

export async function getMediaIndex(
  mediaId: string,
  filterMissingDesc: boolean,
  filterMissingTags: boolean,
): Promise<number | null> {
  return invoke("get_media_index", { mediaId, filterMissingDesc, filterMissingTags });
}

export async function getFilteredMediaIds(
  filterMissingDesc: boolean,
  filterMissingTags: boolean,
): Promise<string[]> {
  return invoke("get_filtered_media_ids", { filterMissingDesc, filterMissingTags });
}

export async function getMediaById(mediaId: string): Promise<MediaInfo | null> {
  return invoke("get_media_by_id", { mediaId });
}

export async function getMissingCounts(): Promise<[number, number, number]> {
  return invoke("get_missing_counts");
}

// --- Tags ---

export interface TagInfo {
  key: string;
  value: string;
}

export interface TagKeyInfo {
  key: string;
  usage_count: number;
}

export async function getMediaTags(mediaId: string): Promise<TagInfo[]> {
  return invoke("get_media_tags", { mediaId });
}

export async function setMediaTags(mediaId: string, tags: TagInfo[]): Promise<void> {
  return invoke("set_media_tags", { mediaId, tags });
}

export async function getAllTagKeys(): Promise<TagKeyInfo[]> {
  return invoke("get_all_tag_keys");
}

export async function getTagValues(key: string): Promise<string[]> {
  return invoke("get_tag_values", { key });
}

export async function createTagKey(key: string): Promise<void> {
  return invoke("create_tag_key", { key });
}

export async function renameTagKey(oldKey: string, newKey: string): Promise<void> {
  return invoke("rename_tag_key", { oldKey, newKey });
}

export async function renameTagValue(
  key: string,
  oldValue: string,
  newValue: string,
): Promise<void> {
  return invoke("rename_tag_value", { key, oldValue, newValue });
}

export async function deleteTagKey(key: string): Promise<void> {
  return invoke("delete_tag_key", { key });
}
