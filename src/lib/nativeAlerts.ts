import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";
import { onAction } from "@tauri-apps/plugin-notification";

/** Traz a janela para frente (desminimiza, mostra e foca). */
export async function focusWindow(): Promise<void> {
  try {
    const w = getCurrentWindow();
    await w.unminimize();
    await w.show();
    await w.setFocus();
  } catch (e) {
    console.error("focusWindow:", e);
  }
}

/** Pisca o ícone na barra de tarefas para chamar atenção (sem roubar foco). */
export async function requestAttention(): Promise<void> {
  try {
    await getCurrentWindow().requestUserAttention(UserAttentionType.Critical);
  } catch (e) {
    console.error("requestUserAttention:", e);
  }
}

/**
 * Ao CLICAR na notificação nativa, traz o app para frente. Retorna uma função
 * para cancelar o registro. Resiliente: se a API não estiver disponível
 * (ex.: fora do Tauri), apenas ignora.
 */
export async function registerNotificationClickFocus(): Promise<() => void> {
  try {
    const listener = await onAction(() => {
      void focusWindow();
    });
    return () => void listener.unregister();
  } catch (e) {
    console.error("registerNotificationClickFocus:", e);
    return () => {};
  }
}
