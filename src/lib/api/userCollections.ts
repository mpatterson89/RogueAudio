import { invoke } from "@tauri-apps/api/core";
import type { UserCollection } from "$lib/types/models";

export const userCollectionsApi = {
  list: (serverId: string, libraryKey: string) =>
    invoke<UserCollection[]>("user_collections_list", { serverId, libraryKey }),
  create: (serverId: string, libraryKey: string, name: string) =>
    invoke<UserCollection>("user_collections_create", { serverId, libraryKey, name }),
  rename: (serverId: string, libraryKey: string, id: string, name: string) =>
    invoke<UserCollection>("user_collections_rename", {
      serverId,
      libraryKey,
      id,
      name,
    }),
  delete: (serverId: string, libraryKey: string, id: string) =>
    invoke<void>("user_collections_delete", { serverId, libraryKey, id }),
  addBooks: (
    serverId: string,
    libraryKey: string,
    id: string,
    ratingKeys: string[],
  ) =>
    invoke<UserCollection>("user_collections_add_books", {
      serverId,
      libraryKey,
      id,
      ratingKeys,
    }),
  removeBooks: (
    serverId: string,
    libraryKey: string,
    id: string,
    ratingKeys: string[],
  ) =>
    invoke<UserCollection>("user_collections_remove_books", {
      serverId,
      libraryKey,
      id,
      ratingKeys,
    }),
  get: (serverId: string, libraryKey: string, id: string) =>
    invoke<UserCollection | null>("user_collections_get", {
      serverId,
      libraryKey,
      id,
    }),
};
