#!/usr/bin/env python3
"""
Conector LOGADO (opcional) do Truth Social para o ESF News Monitor.

Autentica na SUA conta e puxa o home timeline (quem você segue) OU as
publicações de handles específicos. Imprime no stdout um JSON:
    [{"source": "...", "headline": "...", "created_at": "..."}]

O backend Rust executa este script como subprocesso e ingere a saída.

⚠️ AVISOS IMPORTANTES:
  - Acesso automatizado ao Truth Social VIOLA os Termos de Uso deles e pode
    levar à SUSPENSÃO da sua conta. Use por sua conta e risco.
  - O Truth Social usa Cloudflare com impressão digital de TLS; por isso este
    script usa `curl_cffi` (imita o Chrome). Instale com:  pip install curl_cffi
  - Suas credenciais ficam SOMENTE no seu .env local (nunca versionado).

Variáveis de ambiente (do .env):
  TRUTHSOCIAL_USERNAME, TRUTHSOCIAL_PASSWORD   -> login (gera token)
  TRUTHSOCIAL_TOKEN                            -> alternativa: token já pronto
  TRUTHSOCIAL_HANDLES = "realDonaldTrump,..."  -> se vazio, usa o home timeline
  TRUTHSOCIAL_LIMIT   = 20                      -> nº de posts por fonte
"""
import os
import sys
import json
import re

BASE = "https://truthsocial.com"
# Client público do app web do Truth Social (mesmo usado pelo projeto truthbrush).
CLIENT_ID = "9X1Fdd-pxNsAgEDNi_SfhJWi8T-vLuV2WVzKIbkTCw4"
CLIENT_SECRET = "ozF8jzI4968oTKFkEnsBC-UbLPCdrSv0MkXGQu2o_-M"

TAG_RE = re.compile(r"<[^>]+>")


def die(msg: str, code: int = 1):
    print(msg, file=sys.stderr)
    sys.exit(code)


try:
    from curl_cffi import requests  # type: ignore
except Exception:
    die("curl_cffi não instalado. Rode: pip install curl_cffi")


def session():
    # impersonate=chrome -> TLS/JA3 de navegador real (passa pelo Cloudflare)
    return requests.Session(impersonate="chrome")


def strip_html(html: str) -> str:
    text = TAG_RE.sub(" ", html or "")
    text = (
        text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", '"')
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
    )
    return " ".join(text.split()).strip()


def get_token(s) -> str:
    token = os.environ.get("TRUTHSOCIAL_TOKEN", "").strip()
    if token:
        return token
    user = os.environ.get("TRUTHSOCIAL_USERNAME", "").strip()
    pw = os.environ.get("TRUTHSOCIAL_PASSWORD", "").strip()
    if not user or not pw:
        die("Sem TRUTHSOCIAL_TOKEN e sem TRUTHSOCIAL_USERNAME/PASSWORD.")
    r = s.post(
        f"{BASE}/oauth/token",
        json={
            "client_id": CLIENT_ID,
            "client_secret": CLIENT_SECRET,
            "grant_type": "password",
            "username": user,
            "password": pw,
            "redirect_uri": "urn:ietf:wg:oauth:2.0:oob",
            "scope": "read",
        },
        timeout=30,
    )
    if r.status_code != 200:
        die(f"Falha no login (HTTP {r.status_code}): {r.text[:300]}")
    tok = r.json().get("access_token")
    if not tok:
        die("Login sem access_token na resposta.")
    return tok


def to_items(statuses, source_label):
    out = []
    for st in statuses or []:
        content = strip_html(st.get("content", ""))
        if not content:
            continue
        acct = (st.get("account") or {}).get("acct") or ""
        src = source_label or (f"Truth Social (@{acct})" if acct else "Truth Social")
        out.append(
            {
                "source": src,
                "headline": content,
                "created_at": st.get("created_at"),
            }
        )
    return out


def main():
    limit = os.environ.get("TRUTHSOCIAL_LIMIT", "20").strip() or "20"
    handles = [
        h.strip().lstrip("@")
        for h in os.environ.get("TRUTHSOCIAL_HANDLES", "").split(",")
        if h.strip()
    ]

    s = session()
    token = get_token(s)
    headers = {"Authorization": f"Bearer {token}"}

    items = []
    if handles:
        # Publicações de handles específicos
        for h in handles:
            r = s.get(f"{BASE}/api/v1/accounts/lookup", params={"acct": h}, headers=headers, timeout=30)
            if r.status_code != 200:
                print(f"lookup '{h}' falhou (HTTP {r.status_code})", file=sys.stderr)
                continue
            acct_id = r.json().get("id")
            if not acct_id:
                continue
            r2 = s.get(
                f"{BASE}/api/v1/accounts/{acct_id}/statuses",
                params={"exclude_replies": "true", "limit": limit},
                headers=headers,
                timeout=30,
            )
            if r2.status_code == 200:
                items += to_items(r2.json(), f"Truth Social (@{h})")
    else:
        # Home timeline: quem VOCÊ segue
        r = s.get(
            f"{BASE}/api/v1/timelines/home",
            params={"limit": limit},
            headers=headers,
            timeout=30,
        )
        if r.status_code != 200:
            die(f"Home timeline falhou (HTTP {r.status_code}): {r.text[:300]}")
        items = to_items(r.json(), None)

    print(json.dumps(items, ensure_ascii=False))


if __name__ == "__main__":
    main()
