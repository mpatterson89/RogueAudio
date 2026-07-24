/** Build a route to the dedicated book view. */
export function bookHref(serverId: string, ratingKey: string): string {
  return `/book/${encodeURIComponent(serverId)}/${encodeURIComponent(ratingKey)}`;
}

export function authorHref(key: string): string {
  return `/author/${encodeURIComponent(key)}`;
}

/** User collection detail. Plex collections use plex: prefix in id. */
export function collectionHref(id: string, source: "user" | "plex" = "user"): string {
  const pathId = source === "plex" ? `plex:${id}` : id;
  return `/collections/${encodeURIComponent(pathId)}`;
}
