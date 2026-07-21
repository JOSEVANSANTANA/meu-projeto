#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  RECOMENDADOR DE FOTOS
================================================================================
Quando voce envia varias fotos, este modulo pontua cada uma e sugere a mais
adequada para um espaco de foto do template. NAO e um LLM: e uma analise de
qualidade e enquadramento (rapida, offline e explicavel), avaliando:

  - Nitidez (foco): variancia do laplaciano — foto tremida pontua baixo.
  - Resolucao: se a foto tem pixels suficientes para o tamanho do espaco.
  - Enquadramento: quao proxima a proporcao da foto esta da proporcao do espaco
    (menos corte perdido = melhor).
  - Iluminacao: penaliza fotos muito escuras ou estouradas.

Opcionalmente, se a biblioteca de deteccao de rosto estiver disponivel, um
bonus e dado a fotos com um rosto bem posicionado (ver detectar_rosto()).
Sem ela, o sistema funciona normalmente so com as heuristicas acima.
================================================================================
"""

from PIL import Image, ImageOps, ImageFilter

try:
    import numpy as np
    _TEM_NUMPY = True
except ImportError:
    _TEM_NUMPY = False


def _nitidez(img_cinza):
    """Estima o foco pela variancia da borda (quanto maior, mais nitida)."""
    bordas = img_cinza.filter(ImageFilter.FIND_EDGES)
    if _TEM_NUMPY:
        arr = np.asarray(bordas, dtype="float32")
        return float(arr.var())
    # fallback sem numpy: usa a estatistica de extremos do Pillow
    extremos = bordas.getextrema()
    return float(extremos[1] - extremos[0])


def _brilho(img_cinza):
    """Brilho medio 0..255."""
    if _TEM_NUMPY:
        return float(np.asarray(img_cinza, dtype="float32").mean())
    hist = img_cinza.histogram()
    total = sum(hist) or 1
    return sum(i * n for i, n in enumerate(hist)) / total


def pontuar_foto(caminho, box_largura, box_altura):
    """Devolve um dict com a nota (0..100) e os detalhes daquela foto."""
    try:
        img = Image.open(caminho)
        img = ImageOps.exif_transpose(img)
    except Exception as e:
        return {"caminho": caminho, "nota": 0.0, "erro": str(e)}

    largura, altura = img.size
    cinza = img.convert("L")
    # Reduz para acelerar a analise de nitidez em fotos grandes.
    analise = cinza.copy()
    analise.thumbnail((512, 512))

    # --- Resolucao (0..1): a foto cobre bem o espaco? ---
    escala_l = largura / box_largura if box_largura else 1
    escala_a = altura / box_altura if box_altura else 1
    cobertura = min(escala_l, escala_a)
    nota_res = max(0.0, min(1.0, cobertura))  # 1.0 = tem resolucao de sobra

    # --- Enquadramento (0..1): proporcao parecida com a do espaco? ---
    prop_foto = largura / altura if altura else 1
    prop_box = box_largura / box_altura if box_altura else 1
    dif = abs(prop_foto - prop_box) / max(prop_foto, prop_box)
    nota_enq = max(0.0, 1.0 - dif)  # 1.0 = mesma proporcao

    # --- Nitidez (0..1): normalizada por um limiar tipico ---
    nit = _nitidez(analise)
    nota_nit = max(0.0, min(1.0, nit / 800.0))

    # --- Iluminacao (0..1): melhor perto de 128 ---
    br = _brilho(analise)
    nota_luz = max(0.0, 1.0 - abs(br - 128) / 128.0)

    # --- Bonus opcional por rosto ---
    bonus = detectar_rosto(caminho)

    # Peso de cada criterio (soma ~1). Enquadramento e nitidez pesam mais.
    nota = (
        0.30 * nota_enq +
        0.30 * nota_nit +
        0.25 * nota_res +
        0.15 * nota_luz
    ) * 100.0
    nota = min(100.0, nota + bonus)

    return {
        "caminho": caminho,
        "nota": round(nota, 1),
        "resolucao": f"{largura}x{altura}",
        "detalhes": {
            "enquadramento": round(nota_enq, 2),
            "nitidez": round(nota_nit, 2),
            "resolucao": round(nota_res, 2),
            "iluminacao": round(nota_luz, 2),
            "bonus_rosto": bonus,
        },
    }


def detectar_rosto(caminho):
    """Bonus (0..15) se houver um rosto detectavel. Usa OpenCV se instalado;
    caso contrario devolve 0 sem quebrar (deteccao e opcional)."""
    try:
        import cv2  # opcional
        import numpy as np
        dados = np.fromfile(caminho, dtype=np.uint8)
        img = cv2.imdecode(dados, cv2.IMREAD_GRAYSCALE)
        if img is None:
            return 0
        casc = cv2.CascadeClassifier(cv2.data.haarcascades + "haarcascade_frontalface_default.xml")
        rostos = casc.detectMultiScale(img, scaleFactor=1.1, minNeighbors=5)
        return 15 if len(rostos) >= 1 else 0
    except Exception:
        return 0


def recomendar(caminhos, box_largura, box_altura):
    """Pontua uma lista de fotos e devolve tudo ordenado da melhor para a pior.
    O primeiro item e a recomendacao."""
    resultados = [pontuar_foto(c, box_largura, box_altura) for c in caminhos]
    resultados.sort(key=lambda r: r.get("nota", 0), reverse=True)
    return resultados
