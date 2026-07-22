import { invoke } from "@tauri-apps/api/core";
import type {
  AudiobookSummary,
  AuthStatus,
  BookDetail,
  PlaybackInfo,
  PinAuthPoll,
  PinAuthStart,
  PlexLibrary,
  PlexServer,
  StreamInfo,
} from "$lib/types/models";

export const plexApi = {
  startPinAuth: () => invoke<PinAuthStart>("plex_start_pin_auth"),
  pollPinAuth: () => invoke<PinAuthPoll>("plex_poll_pin_auth"),
  logout: () => invoke<void>("plex_logout"),
  authStatus: () => invoke<AuthStatus>("plex_auth_status"),
  devCompleteAuth: (username?: string) =>
    invoke<AuthStatus>("plex_dev_complete_auth", { username: username ?? null }),
  listServers: () => invoke<PlexServer[]>("plex_list_servers"),
  listLibraries: (serverId: string) =>
    invoke<PlexLibrary[]>("plex_list_libraries", { serverId }),
  listBooks: (serverId: string, libraryKey: string, query?: string) =>
    invoke<AudiobookSummary[]>("plex_list_books", {
      serverId,
      libraryKey,
      query: query ?? null,
    }),
  getBookDetail: (serverId: string, ratingKey: string) =>
    invoke<BookDetail>("plex_get_book_detail", { serverId, ratingKey }),
  getPlayback: (serverId: string, ratingKey: string) =>
    invoke<PlaybackInfo>("plex_get_playback", { serverId, ratingKey }),
  getStream: (serverId: string, ratingKey: string) =>
    invoke<StreamInfo>("plex_get_stream", { serverId, ratingKey }),
};
