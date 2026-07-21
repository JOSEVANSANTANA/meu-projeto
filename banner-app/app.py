#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  DASHBOARD - GERADOR DE BANNERS PARA STORIES (Igreja)
================================================================================
Interface grafica (janela) para gerar os banners:
  - Escolha o template (evento/dia) num menu.
  - Envie a(s) foto(s) e digite o(s) nome(s), a data e o horario.
  - Recomendacao automatica: envie varias fotos e o app sugere a melhor.
  - Preveja o resultado e salve o banner (JPG/PNG) na pasta saida/.
  - Gerencie templates: adicione novos (importando o PNG do Canva) ou exclua.

Roda com:  python app.py    (ou pelo executavel .exe depois de empacotar)
================================================================================
"""

import os
import sys
from datetime import datetime

import tkinter as tk
from tkinter import ttk, filedialog, messagebox

from PIL import ImageTk

import engine
import recommender
import templates_store as ts


# --------------------------------------------------------------------------- #
# Diretorio base: funciona tanto rodando o .py quanto o .exe (PyInstaller).
# --------------------------------------------------------------------------- #
def base_dir():
    if getattr(sys, "frozen", False):
        return os.path.dirname(sys.executable)
    return os.path.dirname(os.path.abspath(__file__))


BASE_DIR = base_dir()
PASTA_SAIDA = os.path.join(BASE_DIR, "saida")
PASTA_FOTOS = os.path.join(BASE_DIR, "fotos")

TIPOS_IMAGEM = [("Imagens", "*.jpg *.jpeg *.png *.webp"), ("Todos", "*.*")]


class BannerApp:
    def __init__(self, root):
        self.root = root
        self.store = ts.TemplatesStore(BASE_DIR)
        self.slots_foto = []   # caminho da foto escolhida por espaco
        self.widgets_nome = [] # Entry de cada nome
        self.preview_img = None

        root.title("Gerador de Banners - Stories da Igreja")
        root.geometry("980x720")
        root.minsize(900, 640)

        self._montar_layout()
        self._recarregar_templates()

    # ------------------------------------------------------------------ #
    # Layout
    # ------------------------------------------------------------------ #
    def _montar_layout(self):
        # Barra superior
        topo = ttk.Frame(self.root, padding=10)
        topo.pack(fill="x")
        ttk.Label(topo, text="Gerador de Banners", font=("Segoe UI", 16, "bold")).pack(side="left")
        ttk.Button(topo, text="Gerenciar templates", command=self.abrir_gerenciador).pack(side="right")

        corpo = ttk.Frame(self.root, padding=(10, 0, 10, 10))
        corpo.pack(fill="both", expand=True)

        # Coluna esquerda (formulario) com rolagem
        esquerda_container = ttk.Frame(corpo)
        esquerda_container.pack(side="left", fill="y")
        self.canvas_form = tk.Canvas(esquerda_container, width=440, highlightthickness=0)
        scroll = ttk.Scrollbar(esquerda_container, orient="vertical", command=self.canvas_form.yview)
        self.form = ttk.Frame(self.canvas_form)
        self.form.bind("<Configure>", lambda e: self.canvas_form.configure(scrollregion=self.canvas_form.bbox("all")))
        self.canvas_form.create_window((0, 0), window=self.form, anchor="nw")
        self.canvas_form.configure(yscrollcommand=scroll.set)
        self.canvas_form.pack(side="left", fill="y", expand=False)
        scroll.pack(side="right", fill="y")

        # Template
        ttk.Label(self.form, text="Evento / Dia:", font=("Segoe UI", 10, "bold")).pack(anchor="w", pady=(4, 2))
        self.combo_template = ttk.Combobox(self.form, state="readonly", width=44)
        self.combo_template.pack(anchor="w")
        self.combo_template.bind("<<ComboboxSelected>>", lambda e: self._montar_participantes())

        # Data e horario
        linha_dh = ttk.Frame(self.form)
        linha_dh.pack(anchor="w", fill="x", pady=(10, 4))
        ttk.Label(linha_dh, text="Data:").grid(row=0, column=0, sticky="w")
        self.entry_data = ttk.Entry(linha_dh, width=18)
        self.entry_data.grid(row=1, column=0, padx=(0, 10))
        ttk.Label(linha_dh, text="Horario:").grid(row=0, column=1, sticky="w")
        self.entry_horario = ttk.Entry(linha_dh, width=18)
        self.entry_horario.grid(row=1, column=1)

        # Area dos participantes (montada dinamicamente)
        ttk.Label(self.form, text="Participantes:", font=("Segoe UI", 10, "bold")).pack(anchor="w", pady=(12, 2))
        self.area_participantes = ttk.Frame(self.form)
        self.area_participantes.pack(anchor="w", fill="x")

        # Formato de saida
        linha_fmt = ttk.Frame(self.form)
        linha_fmt.pack(anchor="w", pady=(12, 4))
        ttk.Label(linha_fmt, text="Formato:").pack(side="left")
        self.combo_formato = ttk.Combobox(linha_fmt, state="readonly", width=8, values=["JPG", "PNG"])
        self.combo_formato.set("JPG")
        self.combo_formato.pack(side="left", padx=6)

        # Botoes
        linha_btn = ttk.Frame(self.form)
        linha_btn.pack(anchor="w", pady=(14, 4))
        ttk.Button(linha_btn, text="Gerar preview", command=self.gerar_preview).pack(side="left")
        ttk.Button(linha_btn, text="Salvar banner", command=self.salvar_banner).pack(side="left", padx=8)

        # Coluna direita (preview)
        direita = ttk.Frame(corpo)
        direita.pack(side="left", fill="both", expand=True, padx=(14, 0))
        ttk.Label(direita, text="Pre-visualizacao", font=("Segoe UI", 10, "bold")).pack(anchor="w")
        self.label_preview = ttk.Label(direita, relief="solid", anchor="center")
        self.label_preview.pack(fill="both", expand=True, pady=6)

        # Barra de status
        self.status = tk.StringVar(value="Pronto.")
        ttk.Label(self.root, textvariable=self.status, relief="sunken", anchor="w").pack(fill="x", side="bottom")

    # ------------------------------------------------------------------ #
    # Templates
    # ------------------------------------------------------------------ #
    def _recarregar_templates(self):
        rotulos = [t["rotulo"] for t in self.store.listar()]
        self.combo_template["values"] = rotulos
        if rotulos:
            self.combo_template.set(rotulos[0])
            self._montar_participantes()

    def _template_atual(self):
        return self.store.por_rotulo(self.combo_template.get())

    def _montar_participantes(self):
        """Monta as linhas de foto+nome conforme o numero de espacos do template."""
        for w in self.area_participantes.winfo_children():
            w.destroy()
        self.slots_foto = []
        self.widgets_nome = []

        tpl = self._template_atual()
        if not tpl:
            return
        n_fotos, n_nomes = engine.contar_slots(tpl)
        total = max(n_fotos, n_nomes, 1)

        for i in range(total):
            bloco = ttk.LabelFrame(self.area_participantes, text=f"Pessoa {i + 1}", padding=6)
            bloco.pack(anchor="w", fill="x", pady=4)

            # Nome
            ttk.Label(bloco, text="Nome:").grid(row=0, column=0, sticky="w")
            entry = ttk.Entry(bloco, width=32)
            entry.grid(row=0, column=1, columnspan=2, sticky="w", pady=2)
            self.widgets_nome.append(entry)

            # Foto
            self.slots_foto.append(None)
            lbl = ttk.Label(bloco, text="(nenhuma foto)", foreground="#888")
            lbl.grid(row=1, column=0, columnspan=3, sticky="w")

            ttk.Button(bloco, text="Escolher foto",
                       command=lambda idx=i, l=lbl: self.escolher_foto(idx, l)).grid(row=2, column=0, pady=(4, 0))
            ttk.Button(bloco, text="Recomendar (IA)",
                       command=lambda idx=i, l=lbl: self.recomendar_foto(idx, l)).grid(row=2, column=1, pady=(4, 0), padx=6)

        self.status.set(f"Template '{tpl['rotulo']}': {total} participante(s).")

    # ------------------------------------------------------------------ #
    # Fotos
    # ------------------------------------------------------------------ #
    def escolher_foto(self, idx, lbl):
        inicial = PASTA_FOTOS if os.path.isdir(PASTA_FOTOS) else BASE_DIR
        caminho = filedialog.askopenfilename(title="Escolha a foto", initialdir=inicial, filetypes=TIPOS_IMAGEM)
        if caminho:
            self.slots_foto[idx] = caminho
            lbl.config(text=os.path.basename(caminho), foreground="#000")

    def recomendar_foto(self, idx, lbl):
        """Deixa escolher VARIAS fotos e seleciona automaticamente a melhor."""
        inicial = PASTA_FOTOS if os.path.isdir(PASTA_FOTOS) else BASE_DIR
        caminhos = filedialog.askopenfilenames(
            title="Escolha varias fotos (a IA sugere a melhor)", initialdir=inicial, filetypes=TIPOS_IMAGEM)
        if not caminhos:
            return
        tpl = self._template_atual()
        box = tpl["fotos"][idx] if idx < len(tpl.get("fotos", [])) else {"largura": 500, "altura": 500}
        rank = recommender.recomendar(list(caminhos), box["largura"], box["altura"])
        melhor = rank[0]
        self.slots_foto[idx] = melhor["caminho"]
        lbl.config(text=os.path.basename(melhor["caminho"]) + f"  (nota {melhor['nota']})", foreground="#000")
        # Mostra o ranking resumido
        resumo = "\n".join(
            f"{i+1}. {os.path.basename(r['caminho'])} - nota {r['nota']}" for i, r in enumerate(rank))
        messagebox.showinfo("Recomendacao de foto",
                            f"Escolhi automaticamente:\n{os.path.basename(melhor['caminho'])}\n\nRanking:\n{resumo}")

    # ------------------------------------------------------------------ #
    # Dados do formulario -> dict para o motor
    # ------------------------------------------------------------------ #
    def _coletar_dados(self):
        tpl = self._template_atual()
        if not tpl:
            raise ValueError("Escolha um template.")

        fundo = engine.resolver(BASE_DIR, tpl.get("fundo"))
        if not fundo or not os.path.isfile(fundo):
            raise ValueError(
                f"O fundo do template nao foi encontrado:\n{fundo}\n\n"
                "Exporte o PNG do Canva e coloque na pasta 'templates_bg', "
                "ou use 'Gerenciar templates' para importar.")

        n_fotos, _ = engine.contar_slots(tpl)
        fotos = [self.slots_foto[i] if i < len(self.slots_foto) else None for i in range(n_fotos)]
        for i, f in enumerate(fotos):
            if not f:
                raise ValueError(f"Falta a foto da Pessoa {i + 1}.")
            if not os.path.isfile(f):
                raise ValueError(f"Foto da Pessoa {i + 1} nao encontrada:\n{f}")

        nomes = [w.get().strip() for w in self.widgets_nome]

        return tpl, {
            "fotos": fotos,
            "nomes": nomes,
            "data": self.entry_data.get().strip(),
            "horario": self.entry_horario.get().strip(),
        }

    # ------------------------------------------------------------------ #
    # Preview e salvar
    # ------------------------------------------------------------------ #
    def _render(self):
        tpl, dados = self._coletar_dados()
        canvas = engine.montar_banner(BASE_DIR, self.store.config_geral(), tpl, dados)
        return tpl, dados, canvas

    def gerar_preview(self):
        try:
            _, _, canvas = self._render()
        except Exception as e:
            messagebox.showerror("Nao foi possivel gerar", str(e))
            return
        # Reduz para caber no painel
        prev = canvas.convert("RGB")
        prev.thumbnail((360, 640))
        self.preview_img = ImageTk.PhotoImage(prev)
        self.label_preview.config(image=self.preview_img, text="")
        self.status.set("Pre-visualizacao gerada.")

    def salvar_banner(self):
        try:
            tpl, dados, canvas = self._render()
        except Exception as e:
            messagebox.showerror("Nao foi possivel salvar", str(e))
            return
        os.makedirs(PASTA_SAIDA, exist_ok=True)
        formato = self.combo_formato.get() or "JPG"
        ext = "jpg" if formato.upper() in ("JPG", "JPEG") else "png"
        base_nome = (dados["nomes"][0] if dados["nomes"] and dados["nomes"][0] else tpl["id"]).lower().replace(" ", "_")
        carimbo = datetime.now().strftime("%Y%m%d_%H%M%S")
        destino = os.path.join(PASTA_SAIDA, f"{tpl['id']}_{base_nome}_{carimbo}.{ext}")
        engine.salvar(canvas, destino, formato)
        self.status.set(f"Salvo: {destino}")
        if messagebox.askyesno("Banner salvo", f"Salvo em:\n{destino}\n\nAbrir a pasta?"):
            self._abrir_pasta(PASTA_SAIDA)

    def _abrir_pasta(self, caminho):
        try:
            if sys.platform.startswith("win"):
                os.startfile(caminho)  # type: ignore[attr-defined]
            elif sys.platform == "darwin":
                os.system(f'open "{caminho}"')
            else:
                os.system(f'xdg-open "{caminho}"')
        except Exception:
            pass

    # ------------------------------------------------------------------ #
    # Gerenciar templates
    # ------------------------------------------------------------------ #
    def abrir_gerenciador(self):
        GerenciadorTemplates(self.root, self.store, ao_mudar=self._recarregar_templates)


class GerenciadorTemplates(tk.Toplevel):
    """Janela para adicionar/excluir templates."""

    def __init__(self, parent, store, ao_mudar):
        super().__init__(parent)
        self.store = store
        self.ao_mudar = ao_mudar
        self.title("Gerenciar templates")
        self.geometry("520x420")
        self.transient(parent)

        ttk.Label(self, text="Templates cadastrados", font=("Segoe UI", 11, "bold")).pack(anchor="w", padx=10, pady=(10, 4))
        self.lista = tk.Listbox(self, height=8)
        self.lista.pack(fill="both", expand=True, padx=10)
        ttk.Button(self, text="Excluir selecionado", command=self.excluir).pack(anchor="w", padx=10, pady=6)

        sep = ttk.Separator(self)
        sep.pack(fill="x", pady=8)

        ttk.Label(self, text="Adicionar novo template", font=("Segoe UI", 11, "bold")).pack(anchor="w", padx=10)
        form = ttk.Frame(self, padding=(10, 4))
        form.pack(fill="x")
        ttk.Label(form, text="Nome (rotulo):").grid(row=0, column=0, sticky="w")
        self.e_rotulo = ttk.Entry(form, width=34)
        self.e_rotulo.grid(row=0, column=1, sticky="w", pady=2)
        ttk.Label(form, text="Nº de participantes:").grid(row=1, column=0, sticky="w")
        self.e_num = ttk.Spinbox(form, from_=1, to=8, width=5)
        self.e_num.set(1)
        self.e_num.grid(row=1, column=1, sticky="w", pady=2)
        ttk.Button(form, text="Importar fundo PNG e criar", command=self.adicionar).grid(row=2, column=1, sticky="w", pady=8)

        self._recarregar()

    def _recarregar(self):
        self.lista.delete(0, "end")
        for t in self.store.listar():
            self.lista.insert("end", f"{t['rotulo']}  [{t['id']}]")

    def excluir(self):
        sel = self.lista.curselection()
        if not sel:
            messagebox.showinfo("Selecione", "Escolha um template na lista.")
            return
        tpl = self.store.listar()[sel[0]]
        if messagebox.askyesno("Excluir", f"Excluir '{tpl['rotulo']}'?"):
            self.store.excluir(tpl["id"])
            self._recarregar()
            self.ao_mudar()

    def adicionar(self):
        rotulo = self.e_rotulo.get().strip()
        if not rotulo:
            messagebox.showinfo("Falta o nome", "Digite o nome do template.")
            return
        try:
            num = int(self.e_num.get())
        except ValueError:
            num = 1
        caminho_png = filedialog.askopenfilename(
            title="Escolha o PNG de fundo (exportado do Canva)", filetypes=[("PNG", "*.png"), ("Imagens", "*.jpg *.jpeg *.png")])
        if not caminho_png:
            return
        # id a partir do rotulo
        tid = "".join(c.lower() if c.isalnum() else "_" for c in rotulo).strip("_") or "template"
        fundo_rel = self.store.importar_fundo(caminho_png, tid)
        novo = ts.novo_template(tid, rotulo, fundo_rel, num_fotos=num,
                                largura=self.store.config_geral().get("largura", 1080),
                                altura=self.store.config_geral().get("altura", 1920))
        self.store.adicionar(novo)
        self._recarregar()
        self.ao_mudar()
        messagebox.showinfo(
            "Template criado",
            f"'{rotulo}' criado com {num} participante(s).\n\n"
            "As posicoes de foto/nome vem num padrao inicial. Ajuste fino em "
            "config/templates.json se precisar mover algo.")


def main():
    root = tk.Tk()
    try:
        ttk.Style().theme_use("clam")
    except Exception:
        pass
    BannerApp(root)
    root.mainloop()


if __name__ == "__main__":
    main()
