import { save } from "@tauri-apps/plugin-dialog";
import { api } from "./api";

export async function exportInstanceZip(id: string, name: string): Promise<string | null> {
  const path = await save({
    defaultPath: `${name}.zip`,
    filters: [{ name: "EZMapa instance", extensions: ["zip"] }],
  });
  if (!path) return null;
  await api.exportInstance(id, path);
  return path;
}

// Modpack exports (.mrpack / CurseForge zip) go through the review flow in
// components/PackExport.tsx, which handles the save dialog itself.
