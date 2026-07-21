#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  TESTE DE EXPORTACAO (funciona no Canva Pro)
================================================================================
Confirma se conseguimos PUXAR a sua arte do Canva pela API de exportacao
(diferente do autofill, que exige Enterprise). Exporta um design pelo ID e
baixa as paginas em PNG para a pasta saida/.

Uso:
    python testar_exportacao.py DAHQDuE9X6E
(o ID vem da URL:  canva.com/design/DAHQDuE9X6E/...)
================================================================================
"""

import json
import os
import sys

from canva_client import CanvaClient, CanvaError

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
TOKEN_STORE = os.path.join(BASE_DIR, ".canva_token.json")
PASTA_SAIDA = os.path.join(BASE_DIR, "saida")


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


def extrair_id(entrada):
    """Aceita o ID puro ou uma URL de design e devolve so o ID."""
    entrada = entrada.strip().strip('"')
    if "/design/" in entrada:
        depois = entrada.split("/design/", 1)[1]
        return depois.split("/", 1)[0]
    return entrada


def main():
    if len(sys.argv) < 2:
        print("\nUso: python testar_exportacao.py <design_id ou URL do design>\n")
        sys.exit(1)

    design_id = extrair_id(sys.argv[1])
    print(f"Design ID: {design_id}")

    cid, cs = carregar_env()
    if not cid or not cs:
        print("\n[ERRO] Defina CANVA_CLIENT_ID e CANVA_CLIENT_SECRET (.env).\n")
        sys.exit(1)
    if not os.path.isfile(TOKEN_STORE):
        print("\n[ERRO] Autorize primeiro:  python autorizar.py\n")
        sys.exit(1)

    cliente = CanvaClient(cid, cs, TOKEN_STORE)

    try:
        print("Solicitando exportacao em PNG...")
        resp = cliente._request(
            "POST", "/exports",
            headers={"Content-Type": "application/json"},
            data=json.dumps({"design_id": design_id, "format": {"type": "png"}}),
        )
        if resp.status_code not in (200, 202):
            print(f"\n[FALHA] Export retornou {resp.status_code}: {resp.text}\n")
            sys.exit(1)

        job_id = resp.json()["job"]["id"]
        job = cliente._poll_job(f"/exports/{job_id}", "Exportacao")
        urls = job.get("urls", [])
        if not urls:
            print("\n[FALHA] Exportou mas nao retornou arquivos.\n")
            sys.exit(1)

        os.makedirs(PASTA_SAIDA, exist_ok=True)
        print(f"\n[OK] Exportou {len(urls)} pagina(s). Baixando...")
        for i, u in enumerate(urls, start=1):
            destino = os.path.join(PASTA_SAIDA, f"arte_{design_id}_pagina{i}.png")
            cliente.baixar(u, destino)
            print(f"   -> {destino}")

        print("\n============================================================")
        print("  EXPORTACAO FUNCIONOU! A arte do Canva foi baixada.")
        print("  Abra a pasta 'saida' para ver os PNGs.")
        print("============================================================\n")

    except CanvaError as e:
        print(f"\n[FALHA] {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
