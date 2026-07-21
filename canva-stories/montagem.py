#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  MOTOR DE MONTAGEM (recorte + composicao + textos)
================================================================================
Pega a arte LIMPA (exportada do Canva) e:
  1. Recorta a pessoa da foto (remove o fundo).
       - Fundo VERDE (chroma key): rapido, sem IA, ideal para foto de estudio.
       - Qualquer fundo: usa a biblioteca 'rembg' se instalada (opcional).
  2. Encaixa a pessoa na arte, com mesclagem suave (degrade) na base, para
     imitar o efeito recortado/fundido do seu template.
  3. Escreve nome / data / horario nas posicoes configuradas.

Modulo sem interface: usado pelo provar_montagem.py e, depois, pelo app.
================================================================================
"""

import os

import numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageOps, ImageFilter


# --------------------------------------------------------------------------- #
# 1) Recorte da pessoa (remocao de fundo)
# --------------------------------------------------------------------------- #
def remover_fundo_verde(img, tolerancia=40, limiar_verde=90):
    """Remove fundo verde (chroma key). Devolve RGBA com a pessoa recortada."""
    rgb = img.convert("RGB")
    arr = np.asarray(rgb).astype(np.int16)
    r, g, b = arr[..., 0], arr[..., 1], arr[..., 2]

    # Pixel e "verde de fundo" quando o verde domina claramente r e b.
    eh_verde = (g > limiar_verde) & (g - r > tolerancia) & (g - b > tolerancia)

    alpha = np.where(eh_verde, 0, 255).astype(np.uint8)

    # Suaviza a borda (anti-serrilhado) e come 1px pra tirar a franja verde.
    mask = Image.fromarray(alpha, "L")
    mask = mask.filter(ImageFilter.MinFilter(3))       # encolhe 1px (tira franja)
    mask = mask.filter(ImageFilter.GaussianBlur(1.2))  # borda suave

    # Suaviza o "green spill": reduz o verde residual nas bordas da roupa/pele.
    arr_rgb = np.asarray(rgb).astype(np.int16)
    excesso = (arr_rgb[..., 1] - np.maximum(arr_rgb[..., 0], arr_rgb[..., 2]))
    excesso = np.clip(excesso, 0, 255)
    arr_rgb[..., 1] = arr_rgb[..., 1] - (excesso * 0.6).astype(np.int16)
    arr_rgb = np.clip(arr_rgb, 0, 255).astype(np.uint8)

    out = Image.fromarray(arr_rgb, "RGB").convert("RGBA")
    out.putalpha(mask)
    return out


def remover_fundo_ia(img):
    """Remove fundo de qualquer foto usando rembg (se instalado)."""
    try:
        from rembg import remove
    except ImportError:
        raise RuntimeError(
            "Para fotos sem fundo verde, instale a remocao por IA:\n"
            "   pip install rembg onnxruntime")
    return remove(img.convert("RGBA"))


def recortar_pessoa(caminho_foto, modo="auto"):
    """Abre a foto, corrige rotacao e recorta a pessoa. modo: auto|verde|ia."""
    img = Image.open(caminho_foto)
    img = ImageOps.exif_transpose(img)

    if modo == "verde" or (modo == "auto" and _tem_fundo_verde(img)):
        recorte = remover_fundo_verde(img)
    else:
        recorte = remover_fundo_ia(img)

    return _cortar_para_conteudo(recorte)


def _tem_fundo_verde(img, amostra=40):
    """Heuristica: olha as bordas da imagem; se predominar verde, e chroma key."""
    rgb = img.convert("RGB").resize((100, 100))
    arr = np.asarray(rgb).astype(np.int16)
    bordas = np.concatenate([
        arr[:amostra // 10 + 1, :, :].reshape(-1, 3),
        arr[-(amostra // 10 + 1):, :, :].reshape(-1, 3),
        arr[:, :amostra // 10 + 1, :].reshape(-1, 3),
        arr[:, -(amostra // 10 + 1):, :].reshape(-1, 3),
    ])
    r, g, b = bordas[:, 0], bordas[:, 1], bordas[:, 2]
    verdes = np.mean((g > 90) & (g - r > 30) & (g - b > 30))
    return verdes > 0.5


def _cortar_para_conteudo(rgba):
    """Corta a imagem para a caixa que contem a pessoa (remove vazio ao redor)."""
    alpha = rgba.split()[3]
    caixa = alpha.getbbox()
    return rgba.crop(caixa) if caixa else rgba


# --------------------------------------------------------------------------- #
# 2) Composicao na arte
# --------------------------------------------------------------------------- #
def _fade_base(rgba, altura_fade):
    """Aplica um degrade na BASE do recorte, para 'derreter' na arte."""
    if altura_fade <= 0:
        return rgba
    largura, altura = rgba.size
    grad = np.ones((altura, largura), dtype=np.float32)
    inicio = max(0, altura - altura_fade)
    for i, y in enumerate(range(inicio, altura)):
        grad[y, :] = 1.0 - (i / max(1, altura_fade))
    alpha = np.asarray(rgba.split()[3]).astype(np.float32) * grad
    nova = rgba.copy()
    nova.putalpha(Image.fromarray(alpha.astype(np.uint8), "L"))
    return nova


def compor_pessoa(arte, recorte, x, y, altura_alvo, fade_base=0):
    """Redimensiona o recorte para 'altura_alvo' e cola na arte em (x, y).
    (x, y) e o canto superior esquerdo. Mantem a proporcao da pessoa."""
    prop = recorte.width / recorte.height
    nova_altura = int(altura_alvo)
    nova_largura = int(nova_altura * prop)
    recorte = recorte.resize((nova_largura, nova_altura), Image.LANCZOS)
    recorte = _fade_base(recorte, fade_base)

    base = arte.convert("RGBA")
    base.alpha_composite(recorte, (int(x), int(y)))
    return base


# --------------------------------------------------------------------------- #
# 3) Textos
# --------------------------------------------------------------------------- #
def _fonte(caminho, tam):
    if caminho and os.path.isfile(caminho):
        return ImageFont.truetype(caminho, tam)
    for nome in ("arialbd.ttf", "arial.ttf", "DejaVuSans-Bold.ttf"):
        try:
            return ImageFont.truetype(nome, tam)
        except Exception:
            continue
    return ImageFont.load_default()


def escrever(arte, texto, cfg):
    if not texto:
        return
    d = ImageDraw.Draw(arte)
    conteudo = texto.upper() if cfg.get("maiusculas") else texto
    fonte = _fonte(cfg.get("fonte"), int(cfg.get("tamanho", 46)))
    anchor = {"centro": "mm", "esquerda": "lm", "direita": "rm"}.get(cfg.get("ancora", "centro"), "mm")
    if cfg.get("sombra"):
        d.text((cfg["x"] + 3, cfg["y"] + 3), conteudo, font=fonte, fill="#000000", anchor=anchor)
    d.text((cfg["x"], cfg["y"]), conteudo, font=fonte, fill=cfg.get("cor", "#FFFFFF"), anchor=anchor)


# --------------------------------------------------------------------------- #
# Utilitario: grade para ajudar a achar coordenadas
# --------------------------------------------------------------------------- #
def desenhar_grade(arte, passo=100):
    """Sobrepoe uma grade com coordenadas, para voce achar x/y facilmente."""
    a = arte.convert("RGBA")
    d = ImageDraw.Draw(a)
    for x in range(0, a.width, passo):
        d.line((x, 0, x, a.height), fill=(255, 0, 0, 90))
        d.text((x + 2, 2), str(x), fill=(255, 255, 0, 255))
    for y in range(0, a.height, passo):
        d.line((0, y, a.width, y), fill=(255, 0, 0, 90))
        d.text((2, y + 2), str(y), fill=(255, 255, 0, 255))
    return a


def salvar(arte, destino, formato="PNG", qualidade=95):
    os.makedirs(os.path.dirname(destino) or ".", exist_ok=True)
    if formato.upper() in ("JPG", "JPEG"):
        fundo = Image.new("RGB", arte.size, "#000000")
        fundo.paste(arte, mask=arte.split()[3])
        fundo.save(destino, "JPEG", quality=qualidade)
    else:
        arte.convert("RGBA").save(destino, "PNG")
    return destino
