#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  INSPECIONAR TEMPLATE (descobrir os nomes dos campos)
================================================================================
Mostra os campos variaveis (placeholders) de um Brand Template do Canva, para
voce preencher corretamente a secao "campos" do templates.json.

Uso:
    python inspecionar_template.py DAFxxxxxxxx

Onde DAFxxxxxxxx e o brand_template_id (veja na URL do template no Canva).
================================================================================
"""

import os
import sys

from canva_client import CanvaClient, CanvaError

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
TOKEN_STORE = os.path.join(BASE_DIR, ".canva_token.json")


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
    if len(sys.argv) < 2:
        print("\nUso: python inspecionar_template.py <brand_template_id>\n")
        sys.exit(1)

    template_id = sys.argv[1]
    cid, cs = carregar_env()
    if not cid or not cs:
        print("\n[ERRO] Defina CANVA_CLIENT_ID e CANVA_CLIENT_SECRET (ou use .env).\n")
        sys.exit(1)
    if not os.path.isfile(TOKEN_STORE):
        print("\n[ERRO] Autorize primeiro:  python autorizar.py\n")
        sys.exit(1)

    cliente = CanvaClient(cid, cs, TOKEN_STORE)
    try:
        dataset = cliente.get_dataset(template_id)
    except CanvaError as e:
        print(f"\n[ERRO] {e}\n")
        sys.exit(1)

    if not dataset:
        print("\nEste template nao possui campos variaveis (dataset vazio).")
        print("Verifique se voce marcou as camadas como campos de autofill no Canva.\n")
        return

    print(f"\nCampos do template {template_id}:")
    print("-" * 50)
    for nome_campo, info in dataset.items():
        print(f"  '{nome_campo}'  ->  tipo: {info.get('type')}")
    print("-" * 50)
    print("Use estes nomes (entre aspas) na secao 'campos' do templates.json.\n")


if __name__ == "__main__":
    main()
