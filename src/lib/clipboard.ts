/**
 * Copia texto para a área de transferência. Tenta a API moderna
 * (navigator.clipboard) primeiro; se falhar (ex.: webview sem foco/permissão
 * no momento do clique), cai para o método de fallback via textarea oculto +
 * execCommand, que funciona em praticamente qualquer webview.
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return legacyCopy(text);
  }
}

function legacyCopy(text: string): boolean {
  try {
    const el = document.createElement("textarea");
    el.value = text;
    el.style.position = "fixed";
    el.style.opacity = "0";
    document.body.appendChild(el);
    el.focus();
    el.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(el);
    return ok;
  } catch {
    return false;
  }
}
