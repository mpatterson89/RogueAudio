/** Build a route to the dedicated book view. */
export function bookHref(serverId: string, ratingKey: string): string {
  return `/book/${encodeURIComponent(serverId)}/${encodeURIComponent(ratingKey)}`;
}
