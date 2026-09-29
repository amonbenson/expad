"""Plot the bench measurements for the presentation slides.

Supply rail ripple, from supply_rail_measurement.xlsx: each rail is one group, in the order the
regulators chain them (Vext -> +5V and +3.3V -> +2.5V), with one bar per operating state, in order
of increasing load. Only the 10 ms timebase is plotted: at 1 us the scope's own noise dominates
every reading. Writes supply_rail_ripple.png: the measurements, the scope's noise floor and the
largest rail noise the 7-bit MIDI output tolerates.

Plug-in and unplug timing (M4), from the scope captures in raw_data/ (extracted from
raw_data.zip): jack 1's tip switch and ring around the moment the tip switch opens or closes, with
the monitor's phases marked. Writes plugin_timing_rail_refresh.png (rails older than 30 s, measured
again before the solve), plugin_timing_fresh_rails.png (rails less than 30 s old) and
unplug_timing.png.

Usage (in measurements/): uv run create_plots.py   (dependencies in pyproject.toml)
"""

from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
from openpyxl import load_workbook

HERE = Path(__file__).parent
WORKBOOK = HERE / "supply_rail_measurement.xlsx"
TIMEBASE = "10 ms"

SETUPS = {  # worksheet name -> slide label, in order of increasing load
    "no Microcontroller": "No MCU",
    "Idle/Detecting": "Idle / detecting",
    "Tracking Jack 1": "Tracking",
    "Tracking Jack 1 + WiFi": "Tracking + WiFi",
}
RAILS = {  # worksheet net -> axis label, in regulator chain order
    "Vext": "Vext\n(input)",
    "+5V": "+5V\n(LDO)",
    "+3.3V": "+3.3V\n(LDO)",
    "+2.5V": "+2.5V\n(LDO after 3.3V)",
}
SETUP_COLORS = [
    "#1baf7a",  # aqua
    "#2a78d6",  # blue
    "#4a3aa7",  # violet
    "#eb6834",  # orange: WiFi, the one state that lifts the regulated rails, stands apart
]  # cool hues for increasing load without WiFi; colorblind-safe between neighbouring bars

# Scope noise floor: VRMS1 average of RigolDS2.png, probe tip on its own ground clip, AC coupled,
# 20 MHz bandwidth limit, 10 ms/div, 1 mV/div.
SCOPE_NOISE_FLOOR_RMS = 0.33952e-3

# Largest rail noise the MIDI output tolerates. The ADC reference is the 2.5 V rail
# (board::REFERENCE_VOLTAGE) and a MIDI control change carries 7 bits (MIDI_MAX_VALUE = 127), so a
# full-scale reading splits into 2^7 steps. Gaussian noise stays within +-3 sigma 99.7 % of the time,
# a peak-to-peak span of 6 sigma, and must fit inside one step to not move the sent value:
#
#   V_step    = V_ref / 2^7         = 2.5 V / 128 = 19.5 mV
#   V_rms,max = V_step / 6          = V_ref / (6 * 2^7) = 3.26 mV
REFERENCE_VOLTAGE = 2.5
MIDI_BITS = 7
PEAK_TO_PEAK_PER_RMS = 6
MIDI_STEP_VOLTAGE = REFERENCE_VOLTAGE / 2**MIDI_BITS
MIDI_TOLERABLE_NOISE_RMS = MIDI_STEP_VOLTAGE / PEAK_TO_PEAK_PER_RMS

RAW_DATA = HERE / "raw_data"

# Plug-in and unplug captures: CH1 on J2.TN (tip switch), CH2 on J2.R (ring), 10X probes,
# triggered when the tip switch rises, through 1.9 V as the plug opens it or 0.6 V as the plug
# leaves and it closes. Each capture is (CSV, plotted time range in ms, what the tip switch does at
# 0 ms, phases); each phase is (start in ms, label, inferred). Measured starts are latch edges on the
# ring or tip switch. Inferred ones, drawn dashed, change no drive the two probes can see and are
# one pair's length after the previous latch: Tracking keeps the last pair's drives (14.6 ms), and
# without a plug the tip-sleeve pair only swaps the ring from driven low to floating, both at 0 V
# (13.4 ms, settling for the known pedal).
TIMING_CAPTURES = {
    "plugin_timing_rail_refresh.png": (
        "plugin_w_rail_meas.csv",
        (-20, 340),
        "opens",
        [
            (-20, "Empty", False),
            (0, "Waiting for\nplug check", False),
            (49.4, "Rails:\nall arms high", False),
            (156.3, "Rails:\nall arms low", False),
            (263.2, "Pair tip-ring", False),
            (277.9, "Pair tip-sleeve", False),
            (292.7, "Pair ring-sleeve", False),
            (307.3, "Tracking", True),
        ],
    ),
    "plugin_timing_fresh_rails.png": (
        "plugin5.csv",
        (-20, 130),
        "opens",
        [
            (-20, "Empty", False),
            (0, "Waiting for\nplug check", False),
            (35.5, "Pair tip-ring", False),
            (54.3, "Pair tip-sleeve", False),
            (68.9, "Pair ring-sleeve", False),
            (83.5, "Tracking", True),
        ],
    ),
    "unplug_timing.png": (
        "unplug1.csv",
        (-20, 320),
        "closes",
        [
            (-20, "Tracking", False),
            (1.8, "Rails:\nall arms high", False),
            (108.8, "Rails:\nall arms low", False),
            (215.8, "Pair tip-ring", False),
            (229.2, "Pair tip-sleeve", True),
            (242.6, "Pair ring-sleeve", False),
            (255.9, "Plug check:\nempty", False),
        ],
    ),
}
NARROW_PHASE = 0.15  # share of the plotted time below which a phase's label turns vertical

MILLIVOLTS = 1e3
MILLISECONDS = 1e3
TEXT_COLOR = "#000000"
MUTED_COLOR = "#52514e"
LIMIT_COLOR = "#a4161a"  # deep red, kept apart from the orange WiFi bars
RING_COLOR = "#2a78d6"  # the trace the phases show on
TIP_SWITCH_COLOR = MUTED_COLOR  # context: only the plug-in edge matters
Y_LIMIT = 3.8  # mV, shared by both figures so the slides line up

plt.rcParams.update(
    {
        "font.size": 24,
        "axes.labelsize": 28,
        "xtick.labelsize": 26,
        "ytick.labelsize": 24,
        "legend.fontsize": 24,
        "axes.spines.top": False,
        "axes.spines.right": False,
        "axes.edgecolor": MUTED_COLOR,
        "axes.linewidth": 1.5,
        "axes.axisbelow": True,
        "axes.grid": True,
        "axes.grid.axis": "y",
        "grid.color": "#e4e3df",
        "grid.linewidth": 1.5,
        "text.color": TEXT_COLOR,
        "axes.labelcolor": TEXT_COLOR,
        "xtick.color": TEXT_COLOR,
        "ytick.color": TEXT_COLOR,
    }
)


def load_ripple():
    """Vrms averages in mV as a (rail, setup) array for the plotted timebase."""
    sheet = load_workbook(WORKBOOK, data_only=True).active
    rows = sheet.iter_rows(values_only=True)
    header = next(rows)
    setup_column, division_column, net_column = (
        header.index(name) for name in ("Setup", "Division", "Net")
    )
    rms_column = header.index("Vrms Avg (mV)")

    ripple = np.full((len(RAILS), len(SETUPS)), np.nan)
    for row in rows:
        if (
            row[division_column] == TIMEBASE
            and row[setup_column] in SETUPS
            and row[net_column] in RAILS
        ):
            rail_index = list(RAILS).index(row[net_column])
            setup_index = list(SETUPS).index(row[setup_column])
            ripple[rail_index, setup_index] = row[rms_column]
    if np.isnan(ripple).any():
        raise ValueError(f"{WORKBOOK.name} is missing a {TIMEBASE} rail or setup")
    return ripple


def label_line(axes, text, line_level, text_position, color):
    axes.annotate(
        text,
        xy=(text_position[0], line_level),
        xytext=text_position,
        fontsize=26,
        fontweight="bold",
        color=color,
        ha="center",
        va="bottom",
        arrowprops=dict(arrowstyle="-|>", color=color, linewidth=2, mutation_scale=24),
    )


def plot_supply_rail_ripple(ripple):
    figure, axes = plt.subplots(figsize=(13.33, 7.0))
    group_positions = np.arange(len(RAILS))
    bar_pitch = 0.2
    for setup_index, (label, color) in enumerate(zip(SETUPS.values(), SETUP_COLORS)):
        offsets = (setup_index - (len(SETUPS) - 1) / 2) * bar_pitch
        axes.bar(
            group_positions + offsets,
            ripple[:, setup_index],
            bar_pitch * 0.9,
            color=color,
            label=label,
        )

    noise_floor = SCOPE_NOISE_FLOOR_RMS * MILLIVOLTS
    axes.axhline(noise_floor, color=TEXT_COLOR, linestyle="--", linewidth=3)
    # Between the +3.3V and +2.5V groups, above their bars: the emptiest spot near the line.
    label_line(
        axes,
        f"Oscilloscope noise floor\n{noise_floor:.2f} mV",
        noise_floor,
        (2.5, 1.25),
        TEXT_COLOR,
    )

    midi_limit = MIDI_TOLERABLE_NOISE_RMS * MILLIVOLTS
    axes.axhline(midi_limit, color=LIMIT_COLOR, linewidth=3.5)
    axes.text(
        len(RAILS) - 0.5,
        midi_limit + 0.06,
        f"Max. noise for 7-bit MIDI value: {midi_limit:.2f} mV",
        fontsize=26,
        fontweight="bold",
        color=LIMIT_COLOR,
        ha="right",
        va="bottom",
    )

    axes.set_xticks(group_positions, RAILS.values())
    axes.set_xlim(-0.5, len(RAILS) - 0.5)
    axes.set_ylim(0, Y_LIMIT)
    axes.set_ylabel("Ripple (mV rms)")
    axes.tick_params(axis="x", length=0, pad=10)
    figure.legend(
        loc="upper center",
        ncols=len(SETUPS),
        frameon=False,
        handlelength=1.2,
        columnspacing=1.4,
    )
    figure.tight_layout(rect=(0, 0, 1, 0.92))
    return figure


def load_capture(name):
    """Time in ms, then CH1 and CH2 in V. The CSV rounds its time column, so the time is rebuilt
    from the evenly spaced samples between its first and last entry."""
    data = np.loadtxt(RAW_DATA / name, delimiter=",", skiprows=1)
    time = np.linspace(data[0, 0], data[-1, 0], len(data)) * MILLISECONDS
    return time, data[:, 1], data[:, 2]


def plot_jack_timing(capture, time_range, tip_switch_event, phases):
    time, tip_switch, ring = load_capture(capture)
    shown = (time >= time_range[0]) & (time <= time_range[1])

    figure, (label_axes, tip_switch_axes, ring_axes) = plt.subplots(
        3, 1, sharex=True, figsize=(13.33, 7.5), height_ratios=(1.4, 2, 2)
    )
    traces = (
        (tip_switch_axes, tip_switch, TIP_SWITCH_COLOR, "Tip switch (V)"),
        (ring_axes, ring, RING_COLOR, "Ring (V)"),
    )
    for axes, voltage, color, label in traces:
        # The pull-up rail every contact is driven high to (board::REFERENCE_VOLTAGE).
        axes.axhline(REFERENCE_VOLTAGE, color=MUTED_COLOR, linestyle=":", linewidth=2)
        axes.text(
            time_range[1],
            REFERENCE_VOLTAGE,
            f"+{REFERENCE_VOLTAGE} V supply",
            fontsize=18,
            color=MUTED_COLOR,
            ha="right",
            va="bottom",
            bbox=dict(facecolor="white", edgecolor="none", pad=1),
            zorder=3,  # over the phase lines that cross it
        )
        axes.plot(time[shown], voltage[shown], color=color, linewidth=2)
        axes.set_ylim(-0.2, 3.1)
        axes.set_yticks([0, 1, 2])
        axes.set_ylabel(label, fontsize=22)
    label_axes.axis("off")

    starts = [start for start, _, _ in phases]
    ends = starts[1:] + [time_range[1]]
    plotted_time = time_range[1] - time_range[0]
    for index, ((start, label, inferred), end) in enumerate(zip(phases, ends)):
        if index > 0:
            for axes in (label_axes, tip_switch_axes, ring_axes):
                axes.axvline(
                    start,
                    color=TEXT_COLOR,
                    linewidth=1.5,
                    linestyle="--" if inferred else "-",
                )
        narrow = (end - start) / plotted_time < NARROW_PHASE
        label_axes.text(
            (start + end) / 2,
            0.05,
            label,
            transform=label_axes.get_xaxis_transform(),
            rotation=90 if narrow else 0,
            fontsize=20,
            ha="center",
            va="bottom",
        )

    ring_axes.set_xlim(time_range)
    ring_axes.set_xlabel(f"Time after the tip switch {tip_switch_event} (ms)")
    figure.align_ylabels((tip_switch_axes, ring_axes))
    figure.tight_layout(h_pad=0.4)
    return figure


def save(figure, name):
    figure.savefig(HERE / name, dpi=200, facecolor="white")
    plt.close(figure)
    print(f"wrote {name}")


def main():
    save(plot_supply_rail_ripple(load_ripple()), "supply_rail_ripple.png")
    for name, (capture, time_range, tip_switch_event, phases) in TIMING_CAPTURES.items():
        save(plot_jack_timing(capture, time_range, tip_switch_event, phases), name)


if __name__ == "__main__":
    main()
