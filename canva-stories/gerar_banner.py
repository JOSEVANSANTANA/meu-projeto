#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  GERADOR DE BANNERS VIA CANVA CONNECT API
================================================================================
Fluxo completo (tudo via API oficial do Canva):
  1. Mostra um menu de Evento/Dia (lido de templates.json).
  2. Pede: foto (caminho local), nome, data e horario.
  3. Faz UPLOAD da foto para o Canva (asset).
  4. Chama o AUTOFILL apontando para o Brand Template escolhido, substituindo
     os placeholders (nome, data, horario) e a imagem (foto).
  5. EXPORTA o design gerado (JPG/PNG).
  6. BAIXA o arquivo final para a pasta saida/.

Uso interativo (recomendado):
    python gerar_banner.py

Uso por linha de comando (sem perguntas):
    python gerar_banner.py --evento santa_ceia --nome "Ana" --data "25/12" --horario "19h30" --foto fotos/ana.jpg
================================================================================
"""

import argparse
import json
import os
import sys
from datetime import datetime

from canva_client import CanvaClient, CanvaError

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
TEMPLATES_PATH = os.path.join(BASE_DIR, "templates.json")
TOKEN_STORE = os.path.join(BASE_DIR, ".canva_token.json")
PASTA_SAIDA = os.path.join(BASE_DIR, "saida")


# --------------------------------------------------------------------------- #
# Carregar configuracao e credenciais
# --------------------------------------------------------------------------- #
def carregar_env():
    """Le CANVA_CLIENT_ID/SECRET do ambiente ou de um arquivo .env simples."""
    client_id = os.environ.get("CANVA_CLIENT_ID")
    client_secret = os.environ.get("CANVA_CLIENT_SECRET")

    env_path = os.path.join(BASE_DIR, ".env")
    if (not client_id or not client_secret) and os.path.isfile(env_path):
        with open(env_path, "r", encoding="utf-8") as f:
            for linha in f:
                linha = linha.strip()
                if not linha or linha.startswith("#") or "=" not in linha:
                    continue
                chave, _, valor = linha.partition("=")
                valor = valor.strip().strip('"').strip("'")
                if chave.strip() == "CANVA_CLIENT_ID" and not client_id:
                    client_id = valor
                elif chave.strip() == "CANVA_CLIENT_SECRET" and not client_secret:
                    client_secret = valor

    if not client_id or not client_secret:
        erro_fatal(
            "Credenciais do Canva nao encontradas.\n"
            "       Copie .env.example para .env e preencha CANVA_CLIENT_ID e CANVA_CLIENT_SECRET,\n"
            "       ou defina no CMD:  set CANVA_CLIENT_ID=...  e  set CANVA_CLIENT_SECRET=..."
        )
    return client_id, client_secret


def carregar_templates():
    if not os.path.isfile(TEMPLATES_PATH):
        erro_fatal(f"Arquivo templates.json nao encontrado em {TEMPLATES_PATH}")
    try:
        with open(TEMPLATES_PATH, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        erro_fatal(f"templates.json com formatacao invalida: {e}")


def erro_fatal(mensagem):
    print(f"\n[ERRO] {mensagem}\n")
    sys.exit(1)


# --------------------------------------------------------------------------- #
# Entradas do usuario
# --------------------------------------------------------------------------- #
def escolher_opcao(opcoes):
    """Mostra o menu numerado de eventos/dias e devolve a opcao escolhida."""
    print("\nEscolha o Evento/Dia:")
    for i, op in enumerate(opcoes, start=1):
        print(f"  {i:2d} - {op['rotulo']}")
    while True:
        resposta = input("Numero da opcao: ").strip()
        if resposta.isdigit() and 1 <= int(resposta) <= len(opcoes):
            return opcoes[int(resposta) - 1]
        print(f"  -> Digite um numero de 1 a {len(opcoes)}.")


def buscar_opcao(opcoes, id_evento):
    for op in opcoes:
        if op["id"] == id_evento:
            return op
    return None


def perguntar_texto(rotulo):
    while True:
        valor = input(f"{rotulo}: ").strip()
        if valor:
            return valor
        print("  -> Nao pode ficar em branco.")


def perguntar_foto():
    while True:
        caminho = input("Caminho da foto (ex.: fotos/ana.jpg): ").strip().strip('"')
        if not caminho:
            print("  -> Informe o caminho da foto.")
            continue
        caminho_abs = caminho if os.path.isabs(caminho) else os.path.join(BASE_DIR, caminho)
        if os.path.isfile(caminho_abs):
            return caminho_abs
        print(f"  -> Foto nao encontrada em: {caminho_abs}")


def validar_foto_arg(caminho):
    caminho_abs = caminho if os.path.isabs(caminho) else os.path.join(BASE_DIR, caminho)
    return caminho_abs if os.path.isfile(caminho_abs) else None


# --------------------------------------------------------------------------- #
# Montagem dos dados do autofill
# --------------------------------------------------------------------------- #
def montar_data(campos, asset_id, nome, data, horario):
    """Monta o dict 'data' no formato da API, usando os nomes de campo do
    templates.json (a coluna da direita em 'campos')."""
    return {
        campos["foto"]: {"type": "image", "asset_id": asset_id},
        campos["nome"]: {"type": "text", "text": nome},
        campos["data"]: {"type": "text", "text": data},
        campos["horario"]: {"type": "text", "text": horario},
    }


# --------------------------------------------------------------------------- #
# Programa principal
# --------------------------------------------------------------------------- #
def parse_args():
    p = argparse.ArgumentParser(description="Gera banners de Stories via Canva Connect API.")
    p.add_argument("--evento", help="id do evento/dia (ex.: domingo, santa_ceia)")
    p.add_argument("--nome", help="Nome da pessoa")
    p.add_argument("--data", help="Data do evento (ex.: 25/12)")
    p.add_argument("--horario", help="Horario do evento (ex.: 19h30)")
    p.add_argument("--foto", help="Caminho da foto local")
    return p.parse_args()


def main():
    print("=" * 60)
    print("   GERADOR DE BANNERS - CANVA CONNECT API")
    print("=" * 60)

    config = carregar_templates()
    opcoes = config.get("opcoes", [])
    if not opcoes:
        erro_fatal("Nenhuma opcao configurada em templates.json.")

    args = parse_args()

    # 1) Evento/Dia
    opcao = buscar_opcao(opcoes, args.evento) if args.evento else None
    if args.evento and not opcao:
        print(f"  [AVISO] Evento '{args.evento}' nao existe no templates.json.")
    if not opcao:
        opcao = escolher_opcao(opcoes)

    template_id = opcao.get("brand_template_id", "")
    if not template_id or template_id.startswith("COLE_AQUI"):
        erro_fatal(
            f"O template '{opcao['rotulo']}' ainda nao tem um brand_template_id valido.\n"
            "       Edite templates.json e cole o ID do template da sua conta Canva."
        )

    # 2) Demais inputs
    nome = args.nome.strip() if args.nome and args.nome.strip() else perguntar_texto("Nome da pessoa")
    data = args.data.strip() if args.data and args.data.strip() else perguntar_texto("Data do evento (ex.: 25/12)")
    horario = args.horario.strip() if args.horario and args.horario.strip() else perguntar_texto("Horario (ex.: 19h30)")

    if args.foto:
        caminho_foto = validar_foto_arg(args.foto)
        if not caminho_foto:
            print(f"  [AVISO] Foto '{args.foto}' nao encontrada.")
            caminho_foto = perguntar_foto()
    else:
        caminho_foto = perguntar_foto()

    # Credenciais e cliente
    client_id, client_secret = carregar_env()
    if not os.path.isfile(TOKEN_STORE):
        erro_fatal("Voce ainda nao autorizou o app. Rode primeiro:  python autorizar.py")
    cliente = CanvaClient(client_id, client_secret, TOKEN_STORE)

    print("\nResumo:")
    print(f"  Evento .. {opcao['rotulo']}")
    print(f"  Nome .... {nome}")
    print(f"  Data .... {data}")
    print(f"  Horario . {horario}")
    print(f"  Foto .... {caminho_foto}")

    try:
        print("\n[1/4] Enviando a foto para o Canva...")
        asset_id = cliente.upload_asset(caminho_foto)

        print("[2/4] Preenchendo o template (autofill)...")
        data_dict = montar_data(opcao["campos"], asset_id, nome, data, horario)
        titulo = f"{opcao['rotulo']} - {nome}"
        design_id = cliente.autofill(template_id, data_dict, titulo=titulo)

        print("[3/4] Exportando o design...")
        exp = config.get("export", {})
        urls = cliente.exportar(
            design_id,
            formato=exp.get("formato", "jpg"),
            largura=exp.get("largura", 1080),
            altura=exp.get("altura", 1920),
            qualidade=exp.get("qualidade", 90),
        )

        print("[4/4] Baixando o arquivo final...")
        os.makedirs(PASTA_SAIDA, exist_ok=True)
        formato = exp.get("formato", "jpg").lower()
        ext = "jpg" if formato in ("jpg", "jpeg") else "png"
        nome_arq = nome.lower().replace(" ", "_")
        carimbo = datetime.now().strftime("%Y%m%d_%H%M%S")

        salvos = []
        for i, url in enumerate(urls):
            sufixo = "" if len(urls) == 1 else f"_p{i+1}"
            destino = os.path.join(PASTA_SAIDA, f"{opcao['id']}_{nome_arq}_{carimbo}{sufixo}.{ext}")
            cliente.baixar(url, destino)
            salvos.append(destino)

    except CanvaError as e:
        erro_fatal(str(e))

    print("\n" + "=" * 60)
    print("   BANNER(S) GERADO(S) COM SUCESSO!")
    for s in salvos:
        print(f"   -> {s}")
    print("=" * 60 + "\n")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nOperacao cancelada pelo usuario.\n")
        sys.exit(0)
