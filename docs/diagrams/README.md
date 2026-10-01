# Diagrams

| File | What it is |
|---|---|
| `jack-monitor-states.tex` | Source: TikZ state diagram of `JackMonitor` (topology/src/monitor.rs) and the output stage of expression_controller.rs |
| `jack-monitor-states.pdf` | Vector export for LaTeX documents |
| `jack-monitor-states.svg` | Vector export for PowerPoint and the web (text converted to outlines) |
| `jack-monitor-states.png` | 600 dpi raster fallback |
| `jack-monitor-overview.tex` | Source: the same as four stages in large type, for slides (`.pdf`, `.svg`, `.png` exports alike) |

## Tools

Everything comes with a standard TeX Live installation; the exports were made with:

- **pdflatex**: pdfTeX 3.141592653-2.6-1.40.29 (TeX Live 2026), turning the `.tex` into the PDF
- **pdftocairo** 26.09.0 (Poppler, bundled with TeX Live on Windows, `brew install poppler` on macOS), turning the PDF into the SVG and PNG
- LaTeX packages: `standalone`, TikZ/PGF 3.1.11a (libraries `arrows.meta`, `positioning`, `calc`,
  `shapes.geometric`), `helvet` (Helvetica), `mathastext`, `lmodern`, `amssymb`

## Rebuilding

From this directory, for each diagram (`jack-monitor-states` or `jack-monitor-overview`):

```bash
name=jack-monitor-states
pdflatex $name.tex
pdftocairo -svg $name.pdf $name.svg
pdftocairo -png -r 600 -singlefile $name.pdf $name
```

`pdflatex` also leaves `.aux` and `.log` files behind; delete them, or pass
`-output-directory=<somewhere else>` and copy the PDF back.

## Using the exports

- **LaTeX**: `\includegraphics[width=\linewidth]{jack-monitor-states.pdf}` (it suits a landscape
  page or the full text width).
- **PowerPoint**: *Insert > Pictures* with the SVG. It stays sharp at any size; *Convert to Shape*
  makes its parts editable, but the text stays outlines - edit the `.tex` source for wording.
- **Slides**: the overview is 32 cm wide, about a 16:9 slide, so at that width its type keeps its
  size (18 pt names, 15 pt text). To reveal it step by step, cover each part with a rectangle in
  the slide's background color: every stage is a column that owns the gap to its left with the
  arrows into it, and the two ways back run in their own strips above and below the columns.
