#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  TESTE DE CONEXAO / DIAGNOSTICO
================================================================================
Verifica, em ordem, se tudo esta pronto para gerar banners:
  1. Credenciais (.env) presentes.
  2. Token de autorizacao presente e valido (renova se preciso).
  3. Conexao real com o Canva: lista os Brand Templates da sua conta.
  4. Confere os brand_template_id configurados no templates.json e le os campos
     de cada um (mostrando se batem com o que voce mapeou).

Rode isto ANTES de gerar banners, para ter certeza de que a integracao funciona.

Uso:
    python testar_conexao.py
================================================================================
"""

import json
import os
import sys

from canva_client import CanvaClient, CanvaError, API_BASE

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
TOKEN_STORE = os.path.join(BASE_DIR, ".canva_token.json")
TEMPLATES_PATH = os.path.join(BASE_DIR, "templates.json")


def ok(msg):
    print(f"  [OK]   {msg}")


def falha(msg):
    print(f"  [FALHA] {msg}")


def aviso(msg):
    print(f"  [AVISO] {msg}")


def carregar_env():
    cid = os.environ.get("CANVA_CLIENT_ID")
    cs = os.environ.get("CANVA_CLIENT_SECRET")
    env_path = os.path.join(BASE_DIR, ".env")
    if (not cid or not cs) and os.path.isfile(env_path):
        with open(env_path, "r", encoding="utf-8") as f:
            for linha in f:
                linha = linha.strip()
                if "=" in linha and not linha.startswith("#"):
                    k, _, v = linha.partition("=")
                    v = v.strip().strip('"').strip("'")
                    if k.strip() == "CANVA_CLIENT_ID" and not cid:
                        cid = v
                    elif k.strip() == "CANVA_CLIENT_SECRET" and not cs:
                        cs = v
    return cid, cs


def main():
    print("=" * 60)
    print("   DIAGNOSTICO DA INTEGRACAO CANVA")
    print("=" * 60)

    # ---------------------------------------------------------------- #
    # 1) Credenciais
    # ---------------------------------------------------------------- #
    print("\n1) Credenciais (.env):")
    cid, cs = carregar_env()
    if not cid or not cs or cid.startswith("seu_") or cs.startswith("seu_"):
        falha("CANVA_CLIENT_ID / CANVA_CLIENT_SECRET nao configurados.")
        print("        Copie .env.example para .env e preencha com os seus dados.")
        sys.exit(1)
    ok(f"Client ID encontrado (...{cid[-4:]}).")

    # ---------------------------------------------------------------- #
    # 2) Token
    # ---------------------------------------------------------------- #
    print("\n2) Autorizacao (token):")
    if not os.path.isfile(TOKEN_STORE):
        falha("Nenhum token encontrado. Rode:  python autorizar.py")
        sys.exit(1)
    cliente = CanvaClient(cid, cs, TOKEN_STORE)
    try:
        token = cliente._access_token()  # renova sozinho se estiver expirado
        ok("Token valido (renovado automaticamente se necessario).")
    except CanvaError as e:
        falha(str(e))
        sys.exit(1)

    # ---------------------------------------------------------------- #
    # 3) Conexao real: listar Brand Templates
    # ---------------------------------------------------------------- #
    print("\n3) Conexao com o Canva (listando seus Brand Templates):")
    try:
        resp = cliente._request("GET", "/brand-templates")
        if resp.status_code != 200:
            falha(f"O Canva respondeu {resp.status_code}: {resp.text}")
            if resp.status_code == 403:
                print("        Provavel causa: plano sem acesso a Brand Templates,")
                print("        ou o app nao tem o escopo brandtemplate:meta:read.")
            sys.exit(1)
        itens = resp.json().get("items", [])
        ok(f"Conexao funcionando. {len(itens)} template(s) encontrado(s) na sua conta.")
        templates_conta = {}
        for it in itens:
            templates_conta[it["id"]] = it.get("title", "(sem titulo)")
            print(f"        - {it['id']}  =>  {it.get('title', '')}")
    except CanvaError as e:
        falha(str(e))
        sys.exit(1)

    # ---------------------------------------------------------------- #
    # 4) Conferir os IDs configurados no templates.json
    # ---------------------------------------------------------------- #
    print("\n4) Conferindo os templates configurados (templates.json):")
    if not os.path.isfile(TEMPLATES_PATH):
        aviso("templates.json nao encontrado; pulei esta etapa.")
        return
    config = json.load(open(TEMPLATES_PATH, encoding="utf-8"))
    opcoes = config.get("opcoes", [])
    pendentes, validos = 0, 0

    for op in opcoes:
        tid = op.get("brand_template_id", "")
        rotulo = op.get("rotulo", op.get("id"))
        if not tid or tid.startswith("COLE_AQUI"):
            aviso(f"'{rotulo}': ainda sem brand_template_id (edite o templates.json).")
            pendentes += 1
            continue
        try:
            dataset = cliente.get_dataset(tid)
        except CanvaError as e:
            falha(f"'{rotulo}' ({tid}): {e}")
            continue

        campos_template = set(dataset.keys())
        campos_mapeados = set(op.get("campos", {}).values())
        faltando = campos_mapeados - campos_template
        if faltando:
            falha(f"'{rotulo}': campos do templates.json que NAO existem no template: {sorted(faltando)}")
            print(f"        Campos reais do template: {sorted(campos_template)}")
        else:
            ok(f"'{rotulo}': {len(campos_template)} campos, mapeamento confere.")
            validos += 1

    # ---------------------------------------------------------------- #
    # Resumo
    # ---------------------------------------------------------------- #
    print("\n" + "=" * 60)
    print(f"   RESUMO: {validos} template(s) prontos | {pendentes} pendente(s) de ID")
    if validos and not pendentes:
        print("   Tudo certo! Ja pode rodar:  python gerar_banner.py")
    elif validos:
        print("   Os templates prontos ja funcionam. Configure os pendentes quando quiser.")
    else:
        print("   Nenhum template pronto ainda: cole os brand_template_id no templates.json.")
    print("=" * 60 + "\n")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nCancelado.\n")
        sys.exit(0)
