#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  AUTORIZACAO INICIAL COM O CANVA (rode UMA vez)
================================================================================
Faz o fluxo OAuth 2.0 + PKCE:
  1. Abre o navegador na tela de login/consentimento do Canva.
  2. Voce faz login e clica em "Permitir".
  3. O Canva redireciona para http://127.0.0.1:8080/callback com um "code".
  4. Este script captura o code, troca por tokens e salva em .canva_token.json

Depois disso, o gerar_banner.py renova o acesso sozinho (voce nao repete isso,
a nao ser que revogue o acesso ou fique muitos dias sem usar).

Uso:
    python autorizar.py
================================================================================
"""

import base64
import hashlib
import os
import secrets
import sys
import urllib.parse
import webbrowser
from http.server import BaseHTTPRequestHandler, HTTPServer

from canva_client import (
    AUTORIZAR_URL, ESCOPOS, CanvaClient, CanvaError,
)

# Deve ser EXATAMENTE igual ao Redirect URI cadastrado no Portal do Canva.
REDIRECT_URI = "http://127.0.0.1:8080/callback"
PORTA = 8080

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
TOKEN_STORE = os.path.join(BASE_DIR, ".canva_token.json")


# --------------------------------------------------------------------------- #
# PKCE: gera o par verifier/challenge
# --------------------------------------------------------------------------- #
def gerar_pkce():
    """Cria o code_verifier (segredo) e o code_challenge (hash) do PKCE."""
    verifier = base64.urlsafe_b64encode(secrets.token_bytes(64)).decode("utf-8").rstrip("=")
    verifier = verifier[:128]  # limite de 128 caracteres
    digest = hashlib.sha256(verifier.encode("utf-8")).digest()
    challenge = base64.urlsafe_b64encode(digest).decode("utf-8").rstrip("=")
    return verifier, challenge


# --------------------------------------------------------------------------- #
# Servidor local que recebe o redirecionamento do Canva
# --------------------------------------------------------------------------- #
class CallbackHandler(BaseHTTPRequestHandler):
    codigo = None
    estado_recebido = None
    erro = None

    def do_GET(self):
        query = urllib.parse.urlparse(self.path).query
        params = urllib.parse.parse_qs(query)

        CallbackHandler.codigo = params.get("code", [None])[0]
        CallbackHandler.estado_recebido = params.get("state", [None])[0]
        CallbackHandler.erro = params.get("error", [None])[0]

        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        if CallbackHandler.codigo:
            msg = "<h2>Autorizacao concluida!</h2><p>Pode fechar esta aba e voltar ao CMD.</p>"
        else:
            msg = f"<h2>Falha na autorizacao</h2><p>{CallbackHandler.erro}</p>"
        self.wfile.write(f"<html><body style='font-family:sans-serif'>{msg}</body></html>".encode("utf-8"))

    def log_message(self, *args):
        pass  # silencia o log do servidor


def main():
    client_id = os.environ.get("CANVA_CLIENT_ID")
    client_secret = os.environ.get("CANVA_CLIENT_SECRET")
    if not client_id or not client_secret:
        print(
            "\n[ERRO] Defina as credenciais antes de rodar.\n"
            "       No CMD (mesma janela), rode:\n"
            '         set CANVA_CLIENT_ID=seu_client_id\n'
            '         set CANVA_CLIENT_SECRET=seu_client_secret\n'
            "       (ou copie .env.example para .env e preencha)\n"
        )
        sys.exit(1)

    verifier, challenge = gerar_pkce()
    estado = secrets.token_urlsafe(16)

    params = {
        "response_type": "code",
        "client_id": client_id,
        "redirect_uri": REDIRECT_URI,
        "scope": " ".join(ESCOPOS),
        "code_challenge": challenge,
        "code_challenge_method": "s256",
        "state": estado,
    }
    url = AUTORIZAR_URL + "?" + urllib.parse.urlencode(params)

    print("\nAbrindo o navegador para autorizar o app no Canva...")
    print("Se nao abrir sozinho, copie e cole este endereco no navegador:\n")
    print(url + "\n")
    webbrowser.open(url)

    # Sobe o servidor local e espera o redirecionamento (1 requisicao).
    servidor = HTTPServer(("127.0.0.1", PORTA), CallbackHandler)
    print(f"Aguardando a autorizacao em {REDIRECT_URI} ...")
    servidor.handle_request()
    servidor.server_close()

    if CallbackHandler.erro:
        print(f"\n[ERRO] O Canva retornou: {CallbackHandler.erro}\n")
        sys.exit(1)
    if not CallbackHandler.codigo:
        print("\n[ERRO] Nao recebi o codigo de autorizacao.\n")
        sys.exit(1)
    if CallbackHandler.estado_recebido != estado:
        print("\n[ERRO] Verificacao de seguranca (state) falhou. Tente novamente.\n")
        sys.exit(1)

    print("Codigo recebido. Trocando por token de acesso...")
    cliente = CanvaClient(client_id, client_secret, TOKEN_STORE)
    try:
        cliente.trocar_codigo_por_token(CallbackHandler.codigo, verifier, REDIRECT_URI)
    except CanvaError as e:
        print(f"\n[ERRO] {e}\n")
        sys.exit(1)

    print("\n" + "=" * 60)
    print("   AUTORIZACAO CONCLUIDA COM SUCESSO!")
    print(f"   Token salvo em: {TOKEN_STORE}")
    print("   Agora voce ja pode rodar:  python gerar_banner.py")
    print("=" * 60 + "\n")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nAutorizacao cancelada.\n")
        sys.exit(0)
