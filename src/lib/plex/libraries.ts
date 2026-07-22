import type { PlexLibrary } from "$lib/types/models";

/** Plex audiobook content lives in Music sections (`type` = artist). */
export function isMusicLibrary(lib: PlexLibrary): boolean {
  const t = lib.libraryType.toLowerCase();
  return t === "artist" || t === "music";
}

export function looksLikeAudiobooks(lib: PlexLibrary): boolean {
  const title = lib.title.toLowerCase();
  return /audio|book|spoken/.test(title);
}

/**
 * Prefer music-type libraries only. Sort audiobook-named sections first.
 * Backend already filters; this is a defensive client-side pass.
 */
export function filterMusicLibraries(libraries: PlexLibrary[]): PlexLibrary[] {
  return [...libraries]
    .filter(isMusicLibrary)
    .sort((a, b) => {
      const score = (l: PlexLibrary) => (looksLikeAudiobooks(l) ? 0 : 1);
      const d = score(a) - score(b);
      if (d !== 0) return d;
      return a.title.localeCompare(b.title);
    });
}

/** Default selection when multiple music libraries exist. */
export function pickDefaultLibrary(libraries: PlexLibrary[]): PlexLibrary | null {
  const music = filterMusicLibraries(libraries);
  return music.find(looksLikeAudiobooks) ?? music[0] ?? null;
}
