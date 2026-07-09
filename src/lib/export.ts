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

export async function exportInstanceMrpack(id: string, name: string): Promise<string | null> {
  const path = await save({
    defaultPath: `${name}.mrpack`,
    filters: [{ name: "Modrinth modpack", extensions: ["mrpack"] }],
  });
  if (!path) return null;
  await api.exportMrpack(id, path);
  return path;
}
