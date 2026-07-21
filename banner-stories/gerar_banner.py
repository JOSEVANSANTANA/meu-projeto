#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  GERADOR DE BANNERS PARA STORIES DO INSTAGRAM
================================================================================
Automatiza a criacao de banners para os Stories da igreja.

O que ele faz:
  1. Pergunta (ou recebe por parametro) o dia da semana, o nome da pessoa
     e o caminho da foto.
  2. Recorta/redimensiona a foto para a posicao definida no template SEM
     distorcer (recorte inteligente "cover" + mascara circular opcional).
  3. Escreve o nome com a fonte, cor e posicao padronizadas daquele dia.
  4. Salva a imagem final na pasta "saida/", pronta para postar.

Uso simples (modo interativo - basta dar 2 cliques):
    python gerar_banner.py

Uso avancado (tudo por linha de comando, sem perguntas):
    python gerar_banner.py --dia segunda --nome "Joao Silva" --foto fotos/joao.jpg

Autor: Automacao de Design - Igreja
================================================================================
"""

import argparse
import json
import os
import sys
import unicodedata
from datetime import datetime

# --------------------------------------------------------------------------- #
# Dependencia externa: Pillow (PIL). Se nao estiver instalada, avisamos de
# forma amigavel em vez de mostrar um erro tecnico assustador.
# --------------------------------------------------------------------------- #
try:
    from PIL import Image, ImageDraw, ImageFont, ImageOps
except ImportError:
    print("\n[ERRO] A biblioteca 'Pillow' nao esta instalada.")
    print("       Abra o CMD nesta pasta e rode:  pip install -r requirements.txt")
    print("       (ou:  pip install Pillow)\n")
    sys.exit(1)


# ============================================================================ #
# CONSTANTES / CAMINHOS
# ============================================================================ #
# Diretorio onde este script esta salvo. Usamos isso para que os caminhos
# funcionem mesmo se o usuario rodar o CMD a partir de outra pasta.
BASE_DIR = os.path.dirname(os.path.abspath(__file__))
CONFIG_PATH = os.path.join(BASE_DIR, "config.json")
PASTA_SAIDA = os.path.join(BASE_DIR, "saida")

DIAS_VALIDOS = ["segunda", "terca", "quarta", "quinta", "sexta"]

# Aceitamos escritas variadas para o dia da semana (com/sem acento, abreviado).
ALIASES_DIA = {
    "segunda": "segunda", "seg": "segunda", "segunda-feira": "segunda", "1": "segunda",
    "terca": "terca", "ter": "terca", "terca-feira": "terca", "terça": "terca", "2": "terca",
    "quarta": "quarta", "qua": "quarta", "quarta-feira": "quarta", "3": "quarta",
    "quinta": "quinta", "qui": "quinta", "quinta-feira": "quinta", "4": "quinta",
    "sexta": "sexta", "sex": "sexta", "sexta-feira": "sexta", "5": "sexta",
}


# ============================================================================ #
# UTILITARIOS
# ============================================================================ #
def normalizar(texto):
    """Remove acentos e coloca em minusculo. Ex.: 'Terça-Feira' -> 'terca-feira'."""
    texto = texto.strip().lower()
    texto = unicodedata.normalize("NFKD", texto)
    texto = "".join(c for c in texto if not unicodedata.combining(c))
    return texto


def carregar_config():
    """Le o config.json e valida se ele existe e esta bem formado."""
    if not os.path.isfile(CONFIG_PATH):
        erro_fatal(f"Arquivo de configuracao nao encontrado: {CONFIG_PATH}")
    try:
        with open(CONFIG_PATH, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        erro_fatal(
            "O arquivo config.json esta com erro de formatacao (JSON invalido).\n"
            f"       Detalhe tecnico: {e}"
        )


def erro_fatal(mensagem):
    """Mostra uma mensagem de erro clara e encerra o programa."""
    print(f"\n[ERRO] {mensagem}\n")
    sys.exit(1)


def resolver_caminho(caminho_relativo):
    """Converte um caminho do config (relativo) em caminho absoluto."""
    if os.path.isabs(caminho_relativo):
        return caminho_relativo
    return os.path.join(BASE_DIR, caminho_relativo)


# ============================================================================ #
# ENTRADA DE DADOS (INTERATIVA)
# ============================================================================ #
def perguntar_dia():
    """Pergunta o dia da semana ate receber uma resposta valida."""
    print("\nEscolha o dia da semana:")
    print("  1 - Segunda   2 - Terca   3 - Quarta   4 - Quinta   5 - Sexta")
    while True:
        resposta = input("Dia (nome ou numero): ").strip()
        dia = ALIASES_DIA.get(normalizar(resposta))
        if dia:
            return dia
        print("  -> Opcao invalida. Digite o nome do dia ou o numero (1 a 5).")


def perguntar_nome():
    """Pergunta o nome da pessoa (nao pode ser vazio)."""
    while True:
        nome = input("Nome da pessoa: ").strip()
        if nome:
            return nome
        print("  -> O nome nao pode ficar em branco.")


def perguntar_foto():
    """Pergunta o caminho da foto e valida se o arquivo realmente existe."""
    while True:
        caminho = input("Caminho da foto (ex.: fotos/joao.jpg): ").strip().strip('"')
        if not caminho:
            print("  -> Informe o caminho da foto.")
            continue
        caminho_abs = caminho if os.path.isabs(caminho) else os.path.join(BASE_DIR, caminho)
        if os.path.isfile(caminho_abs):
            return caminho_abs
        print(f"  -> Foto nao encontrada em: {caminho_abs}")
        print("     Confira o nome/caminho e tente de novo.")


# ============================================================================ #
# PROCESSAMENTO DE IMAGEM
# ============================================================================ #
def recortar_cover(foto, largura_alvo, altura_alvo):
    """
    Redimensiona a foto para preencher a caixa (largura x altura) SEM distorcer,
    recortando o excesso e mantendo o centro. Equivale ao 'object-fit: cover'.
    """
    # ImageOps.fit faz exatamente isso: escala mantendo proporcao e corta o
    # excesso, centralizando (centering 0.5, 0.5).
    return ImageOps.fit(
        foto,
        (largura_alvo, altura_alvo),
        method=Image.LANCZOS,
        centering=(0.5, 0.5),
    )


def aplicar_mascara_circular(foto):
    """Deixa a foto redonda (para molduras circulares no template)."""
    largura, altura = foto.size
    # Mascara em alta resolucao (4x) para bordas suaves (antialiasing manual).
    escala = 4
    mascara = Image.new("L", (largura * escala, altura * escala), 0)
    desenho = ImageDraw.Draw(mascara)
    desenho.ellipse((0, 0, largura * escala, altura * escala), fill=255)
    mascara = mascara.resize((largura, altura), Image.LANCZOS)

    foto = foto.convert("RGBA")
    foto.putalpha(mascara)
    return foto


def aplicar_cantos_arredondados(foto, raio):
    """Arredonda os cantos de uma foto retangular."""
    largura, altura = foto.size
    escala = 4
    mascara = Image.new("L", (largura * escala, altura * escala), 0)
    desenho = ImageDraw.Draw(mascara)
    desenho.rounded_rectangle(
        (0, 0, largura * escala, altura * escala),
        radius=raio * escala,
        fill=255,
    )
    mascara = mascara.resize((largura, altura), Image.LANCZOS)

    foto = foto.convert("RGBA")
    foto.putalpha(mascara)
    return foto


def preparar_foto(caminho_foto, cfg_foto):
    """Abre a foto, corrige rotacao do celular, recorta e aplica o formato."""
    try:
        foto = Image.open(caminho_foto)
    except Exception as e:
        erro_fatal(f"Nao foi possivel abrir a foto '{caminho_foto}'.\n       Detalhe: {e}")

    # Corrige a orientacao (fotos de celular guardam a rotacao em metadados EXIF).
    foto = ImageOps.exif_transpose(foto)
    foto = foto.convert("RGBA")

    largura = int(cfg_foto["largura"])
    altura = int(cfg_foto["altura"])
    foto = recortar_cover(foto, largura, altura)

    formato = cfg_foto.get("formato", "retangulo")
    if formato == "circulo":
        foto = aplicar_mascara_circular(foto)
    elif formato in ("arredondado", "retangulo") and cfg_foto.get("raio_borda", 0) > 0:
        foto = aplicar_cantos_arredondados(foto, int(cfg_foto["raio_borda"]))

    return foto


# ============================================================================ #
# TEXTO
# ============================================================================ #
def carregar_fonte(caminho_fonte, tamanho):
    """Carrega a fonte TTF. Se nao achar, usa uma fonte padrao com aviso."""
    caminho_abs = resolver_caminho(caminho_fonte)
    if os.path.isfile(caminho_abs):
        return ImageFont.truetype(caminho_abs, tamanho)
    print(f"  [AVISO] Fonte nao encontrada: {caminho_abs}")
    print("          Usando fonte padrao do sistema (o visual pode diferir).")
    try:
        return ImageFont.truetype("arial.ttf", tamanho)  # comum no Windows
    except Exception:
        return ImageFont.load_default()


def quebrar_linhas(desenho, texto, fonte, largura_max):
    """Quebra o texto em varias linhas para nao ultrapassar a largura maxima."""
    if largura_max <= 0:
        return [texto]
    palavras = texto.split()
    linhas = []
    atual = ""
    for palavra in palavras:
        teste = (atual + " " + palavra).strip()
        largura = desenho.textlength(teste, font=fonte)
        if largura <= largura_max or not atual:
            atual = teste
        else:
            linhas.append(atual)
            atual = palavra
    if atual:
        linhas.append(atual)
    return linhas


def desenhar_texto(canvas, texto, cfg_texto):
    """Escreve o nome no canvas, com quebra de linha, ancoragem e sombra."""
    desenho = ImageDraw.Draw(canvas)

    conteudo = cfg_texto.get("conteudo_prefixo", "") + texto
    if cfg_texto.get("maiusculas"):
        conteudo = conteudo.upper()

    fonte = carregar_fonte(cfg_texto["fonte"], int(cfg_texto["tamanho"]))
    largura_max = int(cfg_texto.get("largura_max", 0))
    linhas = quebrar_linhas(desenho, conteudo, fonte, largura_max)

    x = int(cfg_texto["x"])
    y = int(cfg_texto["y"])
    cor = cfg_texto.get("cor", "#FFFFFF")
    ancora = cfg_texto.get("ancora", "centro")

    # Alinhamento horizontal do bloco de texto.
    align_pillow = {"centro": "center", "esquerda": "left", "direita": "right"}.get(ancora, "center")
    # 'anchor' do Pillow: primeira letra = horizontal, segunda = vertical (topo).
    anchor_pillow = {"centro": "ma", "esquerda": "la", "direita": "ra"}.get(ancora, "ma")

    # Altura de cada linha (com um respiro de 20%).
    ascent, descent = fonte.getmetrics()
    altura_linha = int((ascent + descent) * 1.2)

    texto_multilinha = "\n".join(linhas)

    # Sombra suave para dar legibilidade sobre fundos variados.
    if cfg_texto.get("sombra"):
        desenho.multiline_text(
            (x + 4, y + 4), texto_multilinha, font=fonte, fill="#000000",
            anchor=anchor_pillow, align=align_pillow, spacing=altura_linha - (ascent + descent),
        )

    desenho.multiline_text(
        (x, y), texto_multilinha, font=fonte, fill=cor,
        anchor=anchor_pillow, align=align_pillow, spacing=altura_linha - (ascent + descent),
    )


# ============================================================================ #
# MONTAGEM DO BANNER
# ============================================================================ #
def montar_banner(config, dia, nome, caminho_foto):
    """Junta template + foto + texto e devolve a imagem final (RGBA)."""
    cfg_dia = config["dias"].get(dia)
    if not cfg_dia:
        erro_fatal(f"O dia '{dia}' nao esta configurado no config.json.")

    canvas_cfg = config.get("canvas", {})
    largura_canvas = int(canvas_cfg.get("largura", 1080))
    altura_canvas = int(canvas_cfg.get("altura", 1920))
    cor_fundo = canvas_cfg.get("cor_fundo", "#000000")

    # 1) Base: usa o template PNG do dia. Se nao existir, cria fundo liso e avisa.
    caminho_template = resolver_caminho(cfg_dia["template"])
    if os.path.isfile(caminho_template):
        canvas = Image.open(caminho_template).convert("RGBA")
        # Garante o tamanho de Story (1080x1920) mesmo se o template variar.
        if canvas.size != (largura_canvas, altura_canvas):
            canvas = canvas.resize((largura_canvas, altura_canvas), Image.LANCZOS)
    else:
        print(f"  [AVISO] Template nao encontrado: {caminho_template}")
        print("          Gerando com fundo liso. Coloque o PNG do Canva nesta pasta.")
        canvas = Image.new("RGBA", (largura_canvas, altura_canvas), cor_fundo)

    # 2) Foto da pessoa (recortada/mascarada) colada na posicao do dia.
    cfg_foto = cfg_dia["foto"]
    foto = preparar_foto(caminho_foto, cfg_foto)
    canvas.alpha_composite(foto, (int(cfg_foto["x"]), int(cfg_foto["y"])))

    # 3) Nome por cima.
    desenhar_texto(canvas, nome, cfg_dia["texto"])

    return canvas


def salvar_banner(canvas, dia, nome, formato="JPG"):
    """Salva o banner na pasta saida/ com nome unico (dia + nome + hora)."""
    os.makedirs(PASTA_SAIDA, exist_ok=True)

    nome_arquivo = normalizar(nome).replace(" ", "_") or "sem_nome"
    carimbo = datetime.now().strftime("%Y%m%d_%H%M%S")
    formato = formato.upper()
    extensao = "jpg" if formato in ("JPG", "JPEG") else "png"
    destino = os.path.join(PASTA_SAIDA, f"{dia}_{nome_arquivo}_{carimbo}.{extensao}")

    if extensao == "jpg":
        # JPG nao tem transparencia: achatamos sobre fundo branco.
        fundo = Image.new("RGB", canvas.size, "#FFFFFF")
        fundo.paste(canvas, mask=canvas.split()[3])
        fundo.save(destino, "JPEG", quality=95)
    else:
        canvas.save(destino, "PNG")

    return destino


# ============================================================================ #
# PROGRAMA PRINCIPAL
# ============================================================================ #
def parse_args():
    p = argparse.ArgumentParser(
        description="Gera banners para Stories do Instagram da igreja.",
        formatter_class=argparse.RawTextHelpFormatter,
    )
    p.add_argument("--dia", help="segunda | terca | quarta | quinta | sexta (ou 1 a 5)")
    p.add_argument("--nome", help="Nome da pessoa que aparece no banner")
    p.add_argument("--foto", help="Caminho da foto (ex.: fotos/joao.jpg)")
    p.add_argument("--formato", default="JPG", choices=["JPG", "PNG"], help="Formato de saida (padrao: JPG)")
    return p.parse_args()


def main():
    print("=" * 60)
    print("   GERADOR DE BANNERS - STORIES DA IGREJA")
    print("=" * 60)

    config = carregar_config()
    args = parse_args()

    # Dia: usa o parametro se veio valido; senao, pergunta.
    dia = ALIASES_DIA.get(normalizar(args.dia)) if args.dia else None
    if not dia:
        if args.dia:
            print(f"  [AVISO] Dia '{args.dia}' invalido.")
        dia = perguntar_dia()

    # Nome.
    nome = args.nome.strip() if args.nome and args.nome.strip() else perguntar_nome()

    # Foto (sempre validando a existencia do arquivo).
    if args.foto:
        foto_abs = args.foto if os.path.isabs(args.foto) else os.path.join(BASE_DIR, args.foto)
        if os.path.isfile(foto_abs):
            caminho_foto = foto_abs
        else:
            print(f"  [AVISO] Foto '{args.foto}' nao encontrada.")
            caminho_foto = perguntar_foto()
    else:
        caminho_foto = perguntar_foto()

    print("\nGerando banner...")
    print(f"  Dia .... {dia}")
    print(f"  Nome ... {nome}")
    print(f"  Foto ... {caminho_foto}")

    canvas = montar_banner(config, dia, nome, caminho_foto)
    destino = salvar_banner(canvas, dia, nome, args.formato)

    print("\n" + "=" * 60)
    print("   BANNER GERADO COM SUCESSO!")
    print(f"   Arquivo: {destino}")
    print("=" * 60 + "\n")


if __name__ == "__main__":
    try:
        main()
    except (KeyboardInterrupt, EOFError):
        print("\n\nOperacao cancelada pelo usuario.\n")
        sys.exit(0)
