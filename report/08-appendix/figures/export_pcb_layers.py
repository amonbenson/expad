"""Export the top and bottom copper of the PCB as vector PDFs colored by net.

KiCad's plotter ignores net colors, so this reads the board through pcbnew and writes the
PDFs itself. Run it with KiCad's bundled Python from the repository root:

    "C:/Program Files/KiCad/10.0/bin/python.exe" report/08-appendix/figures/export_pcb_layers.py
"""

import math
import pathlib
import zlib

import pcbnew

REPOSITORY = pathlib.Path(__file__).resolve().parents[3]
BOARD_FILE = REPOSITORY / "hardware" / "ExpressionController.kicad_pcb"
OUTPUT_DIRECTORY = pathlib.Path(__file__).resolve().parent

MARGIN_MM = 1.0
MAX_ERROR_NM = 2000
POINTS_PER_MM = 72 / 25.4

# The project's net colors, in the report's palette (diagram-styles.tex); every other net
# uses SIGNAL_COLOR. Zone fills are drawn as a tint of their net's color.
NET_COLORS = {
    "GND": "0284C7",
    "+2.5V": "D97706",
    "+2.5V_RAW": "92400E",
    "+3.3VA": "E11D48",
    "+3.3VA_RAW": "9F1239",
    "+5V": "C026D3",
    "+5V_RAW": "86198F",
    "VEXT": "CA8A04",
}
SIGNAL_COLOR = "64748B"
ZONE_TINT = 0.22
SUBSTRATE_COLOR = "F8FAFC"
OUTLINE_COLOR = "1E293B"
SILKSCREEN_COLOR = "1E293B"
HOLE_COLOR = "FFFFFF"

SIDES = {
    "top": (pcbnew.F_Cu, pcbnew.F_SilkS, False),
    "bottom": (pcbnew.B_Cu, pcbnew.B_SilkS, True),
}


def tint(color, share):
    channels = [int(color[i : i + 2], 16) for i in (0, 2, 4)]
    tinted = [round(255 - (255 - channel) * share) for channel in channels]
    return "".join(f"{channel:02X}" for channel in tinted)


def add_item(polygons, item, layer):
    item.TransformShapeToPolygon(polygons, layer, 0, MAX_ERROR_NM, pcbnew.ERROR_INSIDE)


def copper_by_color(board, layer):
    """Zone fills (tinted) below tracks, vias and pads, merged per color."""
    zones = {}
    items = {}
    for zone in board.Zones():
        if zone.IsOnLayer(layer) and not zone.GetIsRuleArea():
            color = tint(NET_COLORS.get(zone.GetNetname(), SIGNAL_COLOR), ZONE_TINT)
            zones.setdefault(color, pcbnew.SHAPE_POLY_SET()).Append(zone.GetFilledPolysList(layer))
    copper_items = list(board.GetTracks()) + [pad for footprint in board.GetFootprints() for pad in footprint.Pads()]
    for item in copper_items:
        if item.IsOnLayer(layer):
            color = NET_COLORS.get(item.GetNetname(), SIGNAL_COLOR)
            add_item(items.setdefault(color, pcbnew.SHAPE_POLY_SET()), item, layer)
    return list(zones.items()) + list(items.items())


def silkscreen(board, layer):
    polygons = pcbnew.SHAPE_POLY_SET()
    drawings = list(board.GetDrawings())
    for footprint in board.GetFootprints():
        drawings += list(footprint.GraphicalItems())
        drawings += [field for field in footprint.GetFields() if field.IsVisible()]
    for drawing in drawings:
        if drawing.GetLayer() != layer:
            continue
        if isinstance(drawing, pcbnew.EDA_TEXT):
            drawing.TransformTextToPolySet(polygons, 0, MAX_ERROR_NM, pcbnew.ERROR_INSIDE)
        else:
            add_item(polygons, drawing, layer)
    return polygons


def holes(board):
    polygons = pcbnew.SHAPE_POLY_SET()
    for track in board.GetTracks():
        if track.Type() == pcbnew.PCB_VIA_T:
            polygons.Append(circle(track.GetPosition(), track.GetDrill() // 2))
    for footprint in board.GetFootprints():
        for pad in footprint.Pads():
            if pad.HasHole():
                hole = pad.GetEffectiveHoleShape()
                hole.TransformToPolygon(polygons, MAX_ERROR_NM, pcbnew.ERROR_INSIDE)
    return polygons


def circle(center, radius, segments=32):
    chain = pcbnew.SHAPE_LINE_CHAIN()
    for k in range(segments):
        angle = 2 * math.pi * k / segments
        chain.Append(int(center.x + radius * math.cos(angle)), int(center.y + radius * math.sin(angle)))
    chain.SetClosed(True)
    polygons = pcbnew.SHAPE_POLY_SET()
    polygons.AddOutline(chain)
    return polygons


class Page:
    """A single-page PDF in board coordinates (nm), y pointing down as in KiCad."""

    def __init__(self, box, mirror):
        self.left = box.GetLeft() - pcbnew.FromMM(MARGIN_MM)
        self.right = box.GetRight() + pcbnew.FromMM(MARGIN_MM)
        self.top = box.GetTop() - pcbnew.FromMM(MARGIN_MM)
        self.bottom = box.GetBottom() + pcbnew.FromMM(MARGIN_MM)
        self.mirror = mirror
        self.width = pcbnew.ToMM(self.right - self.left) * POINTS_PER_MM
        self.height = pcbnew.ToMM(self.bottom - self.top) * POINTS_PER_MM
        self.content = []

    def point(self, x, y):
        horizontal = self.right - x if self.mirror else x - self.left
        return (
            pcbnew.ToMM(horizontal) * POINTS_PER_MM,
            pcbnew.ToMM(self.bottom - y) * POINTS_PER_MM,
        )

    def chain(self, chain):
        count = chain.PointCount()
        for k in range(count):
            vertex = chain.CPoint(k)
            x, y = self.point(vertex.x, vertex.y)
            self.content.append(f"{x:.2f} {y:.2f} {'m' if k == 0 else 'l'}")
        self.content.append("h")

    def path(self, polygons):
        polygons.Simplify()
        for index in range(polygons.OutlineCount()):
            self.chain(polygons.Outline(index))
            for hole in range(polygons.HoleCount(index)):
                self.chain(polygons.Hole(index, hole))

    @staticmethod
    def rgb(color):
        return " ".join(f"{int(color[i : i + 2], 16) / 255:.3f}" for i in (0, 2, 4))

    def fill(self, polygons, color):
        if polygons.OutlineCount():
            self.path(polygons)
            self.content.append(f"{self.rgb(color)} rg f*")

    def stroke(self, polygons, color, width_mm):
        self.path(polygons)
        self.content.append(f"{self.rgb(color)} RG {width_mm * POINTS_PER_MM:.2f} w 1 j S")

    def save(self, file):
        stream = zlib.compress("\n".join(self.content).encode())
        objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {self.width:.2f} {self.height:.2f}] "
            "/Contents 4 0 R /Resources << >> >>".encode(),
            f"<< /Length {len(stream)} /Filter /FlateDecode >>\nstream\n".encode() + stream + b"\nendstream",
        ]
        output = bytearray(b"%PDF-1.4\n")
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(len(output))
            output += f"{number} 0 obj\n".encode() + body + b"\nendobj\n"
        cross_reference = len(output)
        output += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
        for offset in offsets:
            output += f"{offset:010d} 00000 n \n".encode()
        output += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{cross_reference}\n%%EOF\n".encode()
        file.write_bytes(output)


def main():
    board = pcbnew.LoadBoard(str(BOARD_FILE))
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    outline = pcbnew.SHAPE_POLY_SET()
    board.GetBoardPolygonOutlines(outline, True)
    drill_holes = holes(board)

    for side, (copper_layer, silkscreen_layer, mirror) in SIDES.items():
        page = Page(board.GetBoardEdgesBoundingBox(), mirror)
        page.fill(outline, SUBSTRATE_COLOR)
        for color, polygons in copper_by_color(board, copper_layer):
            page.fill(polygons, color)
        page.fill(silkscreen(board, silkscreen_layer), SILKSCREEN_COLOR)
        page.fill(drill_holes, HOLE_COLOR)
        page.stroke(outline, OUTLINE_COLOR, 0.25)
        page.save(OUTPUT_DIRECTORY / f"pcb-{side}.pdf")


if __name__ == "__main__":
    main()
