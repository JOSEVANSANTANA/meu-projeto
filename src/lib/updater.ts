import { check } from "@tauri-apps/plugin-updater";

/**
 * Verifica e instala atualizações (item 6 — auto-update).
 * Requer que o app tenha sido publicado com o endpoint + chave de assinatura
 * configurados (ver README). Sem isso, retorna uma mensagem informativa em vez
 * de falhar.
 */
export async function checkForUpdates(): Promise<string> {
  try {
    const update = await check();
    if (!update) return "Você já está na versão mais recente.";
    await update.downloadAndInstall();
    return `Atualização ${update.version} instalada — reinicie o app.`;
  } catch (e) {
    return `Auto-update indisponível/não configurado: ${e}`;
  }
}
