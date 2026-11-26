import type { CatalogFolder, Folder } from "../../ipc/contracts";
import { catalog } from "../../state/catalog.svelte";
import { dialogs } from "../../state/dialogs.svelte";

export function folderEntity(folder: CatalogFolder): Folder | undefined {
  return catalog.snapshot?.folders.find(
    (candidate) => candidate.path.join("/") === folder.path.join("/"),
  );
}

export async function promptNewSubfolder(parent: string[]): Promise<void> {
  const name = await dialogs.input({
    title: "New folder",
    label: "Folder name",
    submitLabel: "Create",
  });
  if (name?.trim()) {
    await catalog.createFolder([...parent, name.trim()]);
  }
}

export async function promptRenameFolder(folder: CatalogFolder): Promise<void> {
  const entity = folderEntity(folder);
  const name = await dialogs.input({
    title: "Rename folder",
    label: "Folder name",
    value: folder.name,
    submitLabel: "Rename",
  });
  if (entity && name?.trim() && name.trim() !== folder.name) {
    await catalog.renameFolder(entity, name.trim());
  }
}

export async function duplicateFolder(folder: CatalogFolder): Promise<void> {
  const entity = folderEntity(folder);
  if (entity) {
    await catalog.duplicateFolder(entity);
  }
}

export async function deleteFolder(folder: CatalogFolder): Promise<void> {
  const entity = folderEntity(folder);
  if (entity) {
    await catalog.deleteFolder(entity);
  }
}
