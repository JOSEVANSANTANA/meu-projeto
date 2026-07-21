#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  PROVA DE MONTAGEM (roda na sua maquina)
================================================================================
Junta tudo do Caminho 2, para voce VER a qualidade:
  1. Puxa a ARTE LIMPA do Canva pelo link (exportacao - funciona no Pro).
  2. Recorta a pessoa da sua foto (fundo verde ou IA).
  3. Encaixa a pessoa na arte com mesclagem suave.
  4. Escreve nome / data / horario.
  5. Salva o resultado em saida/ + uma versao com GRADE para ajustar posicoes.

Exemplo (usando o link da arte limpa e a foto):
  python provar_montagem.py --link https://canva.link/0tnnkk3ec359vqm ^
      --foto minha_foto.jpg --nome "Pr. Thiago" --data "26/07" --horario "19h"

Se ja tiver a arte baixada em PNG, use --arte no lugar de --link:
  python provar_montagem.py --arte saida/arte_limpa.png --foto minha_foto.jpg ...

Ajuste as posicoes com os parametros --px --py --paltura (pessoa) e
--nome-y --data-y (textos). Abra o arquivo *_GRADE.png para achar os numeros.
================================================================================
"""

import argparse
import json
import os
import sys

import requests

import montagem
from canva_client import CanvaClient, CanvaError

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
TOKEN_STORE = os.path.join(BASE_DIR, ".canva_token.json")
PASTA_SAIDA = os.path.join(BASE_DIR, "saida")


# --------------------------------------------------------------------------- #
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
    """Extrai o design_id de uma URL /design/, ou resolve um link canva.link."""
    entrada = entrada.strip().strip('"')
    if "/design/" in entrada:
        return entrada.split("/design/", 1)[1].split("/", 1)[0]
    if "canva.link" in entrada or entrada.startswith("http"):
        # tenta seguir o redirecionamento do link curto ate a URL do design
        try:
            r = requests.get(entrada, allow_redirects=True, timeout=30)
            alvos = [h.headers.get("location", "") for h in r.history] + [r.url]
            for u in alvos:
                if u and "/design/" in u:
                    return u.split("/design/", 1)[1].split("/", 1)[0]
        except requests.RequestException:
            pass
        raise SystemExit(
            "\n[ERRO] Nao consegui descobrir o ID pelo link curto.\n"
            "       Abra o link no navegador, copie a URL completa da barra de\n"
            "       enderecos (canva.com/design/XXXX/...) e use ela no --link.\n")
    return entrada  # ja e um id


def exportar_arte(cliente, design_id, pagina):
    """Exporta a arte em PNG e baixa a pagina escolhida. Devolve o caminho."""
    resp = cliente._request(
        "POST", "/exports",
        headers={"Content-Type": "application/json"},
        data=json.dumps({"design_id": design_id, "format": {"type": "png"}}),
    )
    if resp.status_code not in (200, 202):
        raise CanvaError(f"Export retornou {resp.status_code}: {resp.text}")
    job_id = resp.json()["job"]["id"]
    job = cliente._poll_job(f"/exports/{job_id}", "Exportacao")
    urls = job.get("urls", [])
    if not urls:
        raise CanvaError("Exportou mas nao retornou arquivos.")
    idx = min(max(1, pagina), len(urls)) - 1
    os.makedirs(PASTA_SAIDA, exist_ok=True)
    destino = os.path.join(PASTA_SAIDA, f"arte_limpa_{design_id}.png")
    cliente.baixar(urls[idx], destino)
    return destino


# --------------------------------------------------------------------------- #
def main():
    p = argparse.ArgumentParser(description="Prova de montagem (Caminho 2).")
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--link", help="Link/URL da arte limpa no Canva")
    g.add_argument("--arte", help="Caminho de um PNG de arte ja baixado")
    p.add_argument("--foto", required=True, help="Foto da pessoa (fundo verde de preferencia)")
    p.add_argument("--nome", default="")
    p.add_argument("--data", default="")
    p.add_argument("--horario", default="")
    p.add_argument("--pagina", type=int, default=1, help="Pagina da arte (se tiver varias)")
    p.add_argument("--modo", default="auto", choices=["auto", "verde", "ia"], help="Recorte da foto")
    # posicao da pessoa
    p.add_argument("--px", type=int, default=None, help="X da pessoa (padrao: centralizado)")
    p.add_argument("--py", type=int, default=None, help="Y (topo) da pessoa")
    p.add_argument("--paltura", type=int, default=None, help="Altura da pessoa em px")
    p.add_argument("--fade", type=int, default=160, help="Altura do degrade na base da pessoa")
    # posicao dos textos (Y; X = centro por padrao)
    p.add_argument("--nome-y", type=int, default=None)
    p.add_argument("--data-y", type=int, default=None)
    args = p.parse_args()

    # 1) Obter a arte
    if args.arte:
        if not os.path.isfile(args.arte):
            sys.exit(f"\n[ERRO] Arte nao encontrada: {args.arte}\n")
        arte_path = args.arte
    else:
        cid, cs = carregar_env()
        if not cid or not cs:
            sys.exit("\n[ERRO] Defina CANVA_CLIENT_ID/SECRET no .env.\n")
        if not os.path.isfile(TOKEN_STORE):
            sys.exit("\n[ERRO] Autorize primeiro:  python autorizar.py\n")
        design_id = extrair_id(args.link)
        print(f"Design ID: {design_id}\nExportando a arte limpa do Canva...")
        try:
            arte_path = exportar_arte(CanvaClient(cid, cs, TOKEN_STORE), design_id, args.pagina)
        except CanvaError as e:
            sys.exit(f"\n[ERRO] {e}\n")
        print(f"Arte baixada: {arte_path}")

    if not os.path.isfile(args.foto):
        sys.exit(f"\n[ERRO] Foto nao encontrada: {args.foto}\n")

    from PIL import Image
    arte = Image.open(arte_path).convert("RGBA")
    L, A = arte.size
    print(f"Arte: {L}x{A}")

    # 2) Recortar a pessoa
    print("Recortando a pessoa da foto...")
    try:
        recorte = montagem.recortar_pessoa(args.foto, modo=args.modo)
    except RuntimeError as e:
        sys.exit(f"\n[ERRO] {e}\n")

    # 3) Posicoes (padroes razoaveis; ajuste com os parametros)
    paltura = args.paltura or int(A * 0.62)
    prop = recorte.width / recorte.height
    plargura = int(paltura * prop)
    px = args.px if args.px is not None else (L - plargura) // 2
    py = args.py if args.py is not None else int(A * 0.20)

    composto = montagem.compor_pessoa(arte, recorte, px, py, paltura, fade_base=args.fade)

    # 4) Textos
    nome_y = args.nome_y if args.nome_y is not None else int(A * 0.72)
    data_y = args.data_y if args.data_y is not None else int(A * 0.80)
    montagem.escrever(composto, args.nome, {"x": L // 2, "y": nome_y, "tamanho": 78,
                      "cor": "#FFFFFF", "maiusculas": True, "sombra": True, "ancora": "centro"})
    montagem.escrever(composto, args.data, {"x": int(L * 0.34), "y": data_y, "tamanho": 50,
                      "cor": "#FFFFFF", "sombra": True, "ancora": "centro"})
    montagem.escrever(composto, args.horario, {"x": int(L * 0.66), "y": data_y, "tamanho": 50,
                      "cor": "#FFFFFF", "sombra": True, "ancora": "centro"})

    # 5) Salvar resultado + grade
    os.makedirs(PASTA_SAIDA, exist_ok=True)
    saida = os.path.join(PASTA_SAIDA, "PROVA_montagem.jpg")
    montagem.salvar(composto, saida, "JPG")
    grade = os.path.join(PASTA_SAIDA, "PROVA_GRADE.png")
    montagem.salvar(montagem.desenhar_grade(arte), grade, "PNG")

    print("\n============================================================")
    print(f"  PRONTO! Resultado:  {saida}")
    print(f"  Grade p/ ajustar:   {grade}")
    print("  Se a pessoa/texto sairem fora do lugar, olhe a GRADE e rode de")
    print("  novo com --px --py --paltura --nome-y --data-y.")
    print("============================================================\n")


if __name__ == "__main__":
    main()
