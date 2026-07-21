#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  MOTOR DE RENDERIZACAO DOS BANNERS (Pillow)
================================================================================
Responsavel por montar a imagem final a partir de:
  - um fundo (PNG exportado do Canva),
  - uma ou mais fotos (recortadas sem distorcer, com mascara opcional),
  - um ou mais textos (nome(s), data, horario) com fonte/cor/posicao.

Suporta VARIOS participantes: um template pode ter N espacos de foto e N nomes.

Este modulo NAO depende de interface grafica: pode ser usado pelo dashboard
(app.py) ou por scripts. Isso facilita testar e reaproveitar.
================================================================================
"""

import os

from PIL import Image, ImageDraw, ImageFont, ImageOps


# --------------------------------------------------------------------------- #
# Utilitarios de caminho
# --------------------------------------------------------------------------- #
def resolver(base_dir, caminho):
    """Converte um caminho relativo (do templates.json) em absoluto."""
    if not caminho:
        return caminho
    return caminho if os.path.isabs(caminho) else os.path.join(base_dir, caminho)


# --------------------------------------------------------------------------- #
# Foto: recorte "cover" + mascaras
# --------------------------------------------------------------------------- #
def recortar_cover(foto, largura, altura):
    """Preenche a caixa mantendo a proporcao (sem distorcer) e cortando o
    excesso, centralizado. Equivale ao 'object-fit: cover' do CSS."""
    return ImageOps.fit(foto, (largura, altura), method=Image.LANCZOS, centering=(0.5, 0.5))


def _mascara_suave(desenhar_forma, largura, altura, escala=4):
    """Cria uma mascara em alta resolucao e reduz, para bordas suaves."""
    mascara = Image.new("L", (largura * escala, altura * escala), 0)
    d = ImageDraw.Draw(mascara)
    desenhar_forma(d, escala)
    return mascara.resize((largura, altura), Image.LANCZOS)


def aplicar_formato(foto, formato, raio_borda=0):
    """Aplica mascara circular, cantos arredondados ou nada (retangulo)."""
    largura, altura = foto.size
    foto = foto.convert("RGBA")

    if formato == "circulo":
        mascara = _mascara_suave(
            lambda d, e: d.ellipse((0, 0, largura * e, altura * e), fill=255),
            largura, altura,
        )
        foto.putalpha(mascara)
    elif formato == "arredondado" and raio_borda > 0:
        mascara = _mascara_suave(
            lambda d, e: d.rounded_rectangle(
                (0, 0, largura * e, altura * e), radius=raio_borda * e, fill=255),
            largura, altura,
        )
        foto.putalpha(mascara)
    return foto


def preparar_foto(caminho_foto, slot):
    """Abre a foto, corrige rotacao EXIF, recorta e aplica o formato do slot."""
    foto = Image.open(caminho_foto)
    foto = ImageOps.exif_transpose(foto)  # corrige fotos de celular
    foto = foto.convert("RGBA")
    foto = recortar_cover(foto, int(slot["largura"]), int(slot["altura"]))
    return aplicar_formato(foto, slot.get("formato", "retangulo"), int(slot.get("raio_borda", 0)))


# --------------------------------------------------------------------------- #
# Texto
# --------------------------------------------------------------------------- #
def carregar_fonte(caminho_fonte, tamanho):
    if caminho_fonte and os.path.isfile(caminho_fonte):
        return ImageFont.truetype(caminho_fonte, tamanho)
    try:
        return ImageFont.truetype("arial.ttf", tamanho)
    except Exception:
        return ImageFont.load_default()


def quebrar_linhas(desenho, texto, fonte, largura_max):
    if not largura_max or largura_max <= 0:
        return [texto]
    linhas, atual = [], ""
    for palavra in texto.split():
        teste = (atual + " " + palavra).strip()
        if desenho.textlength(teste, font=fonte) <= largura_max or not atual:
            atual = teste
        else:
            linhas.append(atual)
            atual = palavra
    if atual:
        linhas.append(atual)
    return linhas


def desenhar_texto(canvas, texto, cfg, fonte_padrao):
    """Escreve um texto no canvas conforme a configuracao (posicao/cor/etc.)."""
    if texto is None or texto == "":
        return
    desenho = ImageDraw.Draw(canvas)

    conteudo = str(cfg.get("prefixo", "")) + str(texto)
    if cfg.get("maiusculas"):
        conteudo = conteudo.upper()

    caminho_fonte = cfg.get("fonte") or fonte_padrao
    fonte = carregar_fonte(caminho_fonte, int(cfg.get("tamanho", 60)))
    linhas = quebrar_linhas(desenho, conteudo, fonte, int(cfg.get("largura_max", 0)))
    texto_multi = "\n".join(linhas)

    x, y = int(cfg["x"]), int(cfg["y"])
    cor = cfg.get("cor", "#FFFFFF")
    ancora = cfg.get("ancora", "centro")
    align = {"centro": "center", "esquerda": "left", "direita": "right"}.get(ancora, "center")
    anchor = {"centro": "ma", "esquerda": "la", "direita": "ra"}.get(ancora, "ma")

    ascent, descent = fonte.getmetrics()
    spacing = int((ascent + descent) * 0.2)

    if cfg.get("sombra"):
        desenho.multiline_text((x + 4, y + 4), texto_multi, font=fonte, fill="#000000",
                               anchor=anchor, align=align, spacing=spacing)
    desenho.multiline_text((x, y), texto_multi, font=fonte, fill=cor,
                           anchor=anchor, align=align, spacing=spacing)


# --------------------------------------------------------------------------- #
# Montagem do banner
# --------------------------------------------------------------------------- #
def montar_banner(base_dir, config_geral, template, dados):
    """
    Monta o banner e devolve a imagem (RGBA).

    dados = {
        "fotos":   [caminho1, caminho2, ...],   # 1 por espaco de foto do template
        "nomes":   ["Ana", "Joao", ...],        # 1 por campo de nome do template
        "data":    "25/12",
        "horario": "19h30",
    }
    """
    largura = int(config_geral.get("largura", 1080))
    altura = int(config_geral.get("altura", 1920))
    fonte_padrao = resolver(base_dir, config_geral.get("fonte_padrao"))

    # 1) Fundo
    fundo_path = resolver(base_dir, template.get("fundo"))
    if fundo_path and os.path.isfile(fundo_path):
        canvas = Image.open(fundo_path).convert("RGBA")
        if canvas.size != (largura, altura):
            canvas = canvas.resize((largura, altura), Image.LANCZOS)
    else:
        canvas = Image.new("RGBA", (largura, altura), config_geral.get("cor_fundo", "#111111"))

    # 2) Fotos (varios participantes)
    fotos = dados.get("fotos", []) or []
    for i, slot in enumerate(template.get("fotos", [])):
        if i >= len(fotos) or not fotos[i]:
            continue
        foto = preparar_foto(fotos[i], slot)
        canvas.alpha_composite(foto, (int(slot["x"]), int(slot["y"])))

    # 3) Textos (nomes em ordem + data + horario + fixos)
    nomes = list(dados.get("nomes", []) or [])
    idx_nome = 0
    for cfg in template.get("textos", []):
        origem = cfg.get("origem", "fixo")
        if origem == "nome":
            valor = nomes[idx_nome] if idx_nome < len(nomes) else ""
            idx_nome += 1
        elif origem == "data":
            valor = dados.get("data", "")
        elif origem == "horario":
            valor = dados.get("horario", "")
        else:  # fixo
            valor = cfg.get("texto", "")
        desenhar_texto(canvas, valor, cfg, fonte_padrao)

    return canvas


def salvar(canvas, destino, formato="JPG", qualidade=95):
    """Salva o canvas em JPG (achatado sobre branco) ou PNG."""
    os.makedirs(os.path.dirname(destino) or ".", exist_ok=True)
    formato = formato.upper()
    if formato in ("JPG", "JPEG"):
        fundo = Image.new("RGB", canvas.size, "#FFFFFF")
        fundo.paste(canvas, mask=canvas.split()[3])
        fundo.save(destino, "JPEG", quality=qualidade)
    else:
        canvas.save(destino, "PNG")
    return destino


def contar_slots(template):
    """Devolve (num_fotos, num_nomes) que o template espera."""
    n_fotos = len(template.get("fotos", []))
    n_nomes = sum(1 for t in template.get("textos", []) if t.get("origem") == "nome")
    return n_fotos, n_nomes
