#!/usr/bin/env python3
"""Extracts the oscillator core's timing parameters from the circuit reference.

Input: circuits/boards/reference/vco1-core.json (`ca72-lab core`): per note, the period,
the timing current at two ramp voltages, the ramp's extremes.
Output: the constants of ca72::vco::CoreParams, printed as Rust.

The period of a note is the ramp's run from its top down to its bottom (C dv/dt =
-(I(v) - I_leak), I linear in v through the Early effect) plus a hold while the reset
transistor releases. Least squares over every note gives the capacitance the ramp node
adds to C1 (junction capacitances), the hold time and a leakage current (ngspice's gmin
across the junctions). The comparator's overshoot below its threshold is tabulated
against the ramp's slope; the reset's top voltage falls linearly with slope.
"""
import json, sys
import numpy as np
from scipy.optimize import least_squares

path = sys.argv[1] if len(sys.argv) > 1 else "circuits/boards/reference/vco1-core.json"
d = json.load(open(path))
C1 = 0.01e-6
P = []
for p in d["points"]:
    b = (p["i_hi"] - p["i_lo"]) / 3.6
    a = p["i_hi"] + 0.2 * b
    P.append((p["range"], p["key"], 1 / p["hz"], a, b, p["ramp_min"], p["ramp_max"]))

def period(x, p):
    k, th, ilk = x
    _, _, _, a, b, vmin, vtop = p
    return C1 * (1 + k) / b * np.log((a + b * vtop - ilk) / (a + b * vmin - ilk)) + th

def res(x):
    return np.array([1200 * np.log2(period(x, p) / p[2]) for p in P])

fit = least_squares(res, [0.0017, 1.2e-6, 1e-14], x_scale=[1e-3, 1e-6, 1e-14])
k, t_hold, i_leak = fit.x
c_eff = C1 * (1 + k)
vth0 = min(P, key=lambda p: p[2] and 1 / p[2])[5]
slow = max(P, key=lambda p: p[2])
vth0 = slow[5]
rows = []
for p in P:
    _, _, T, a, b, vmin, vtop = p
    s = (a + b * vmin - i_leak) / c_eff  # V/s at the bottom
    rows.append((s, (vth0 - vmin) / s, vtop))
rows.sort()
# Top voltage: vtop = vtop0 - tau * s
S = np.array([r[0] for r in rows]); VT = np.array([r[2] for r in rows])
tau, vtop0 = np.polyfit(S, VT, 1)
# Delay table on the musical ranges' slopes (above 50 V/s), 8 points in log s.
mus = [(s, dly) for s, dly, _ in rows if s > 50]
ls = np.log([m[0] for m in mus]); dl = np.array([m[1] for m in mus])
grid = np.linspace(ls.min(), ls.max(), 8)
dgrid = np.interp(grid, ls, dl)
print(f"// fit residual: max {abs(res(fit.x)).max():.4f} cents over {len(P)} notes")
print(f"c_par: {c_eff - C1:.6e},")
print(f"t_hold: {t_hold:.6e},")
print(f"i_leak: {i_leak:.6e},")
print(f"v_threshold: {vth0:.6f},")
print(f"v_top0: {vtop0:.6f},")
print(f"top_per_slope: {-tau:.6e},")
print("delay_ln_slope: [" + ", ".join(f"{g:.6}" for g in grid) + "],")
print("delay: [" + ", ".join(f"{x:.6e}" for x in dgrid) + "],")
