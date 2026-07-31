import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";

/**
 * Traz a janela para frente (desminimiza, mostra e foca).
 *
 * O clique na notificação nativa é tratado inteiramente no backend Rust
 * (ver `fire_native_alert` em `engine.rs`): a notificação é disparada via
 * `notify-rust` diretamente (não via `@tauri-apps/plugin-notification`, cujo
 * evento de clique nunca é emitido no desktop) e, ao detectar o clique, o
 * próprio Rust chama `unminimize/show/set_focus` na janela — sem round-trip
 * ao JS. Esta função continua existindo para outros usos (ex.: focar a janela
 * a partir de outra ação da UI), mas não participa mais do fluxo de clique
 * na notificação.
 */
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
