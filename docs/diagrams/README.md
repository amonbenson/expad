# Diagrams

| File | What it is |
|---|---|
| `jack-monitor-states.tex` | Source: TikZ state diagram of `JackMonitor` (topology/src/monitor.rs) and the output stage of expression_controller.rs |
| `jack-monitor-states.pdf` | Vector export for LaTeX documents |
| `jack-monitor-states.svg` | Vector export for PowerPoint and the web (text converted to outlines) |
| `jack-monitor-states.png` | 600 dpi raster fallback |

## Tools

Everything comes with a standard TeX Live installation; the exports were made with:

- **pdflatex**: pdfTeX 3.141592653-2.6-1.40.28 (TeX Live 2025), turning the `.tex` into the PDF
- **pdftocairo** 25.02.0 (Poppler, bundled with TeX Live on Windows), turning the PDF into the SVG and PNG
- LaTeX packages: `standalone`, TikZ/PGF 3.1.11 (libraries `arrows.meta`, `positioning`, `calc`,
  `shapes.geometric`), `FiraSans`/`FiraMono` (fonts), `lmodern`, `amssymb`

## Rebuilding

From this directory:

```bash
pdflatex jack-monitor-states.tex
pdftocairo -svg jack-monitor-states.pdf jack-monitor-states.svg
pdftocairo -png -r 600 -singlefile jack-monitor-states.pdf jack-monitor-states
```

`pdflatex` also leaves `.aux` and `.log` files behind; delete them, or pass
`-output-directory=<somewhere else>` and copy the PDF back.

## Using the exports

- **LaTeX**: `\includegraphics[width=\linewidth]{jack-monitor-states.pdf}` (it suits a landscape
  page or the full text width).
- **PowerPoint**: *Insert > Pictures* with the SVG. It stays sharp at any size; *Convert to Shape*
  makes its parts editable, but the text stays outlines - edit the `.tex` source for wording.
