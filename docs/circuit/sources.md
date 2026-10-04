# Sources

Every file's URL, sha256 and type is in [`sources/manifest.tsv`](sources/manifest.tsv) (234
files, gathered 2026-09-28). The files themselves are not in the repository: several sites
ask that their scans not be re-hosted, and vendor SPICE models may not be redistributed.
`scripts/fetch-sources.sh` downloads them into `~/.cache/ca72-sources` and
checks each hash.

Tags (S-...) are how the other documents cite a source. "Scan" gives the best copy found.

## Schematics and service documents (primary)

| Tag | Document | Date | Scan | Used for |
|---|---|---|---|---|
| S-F93 | Service manual Figure 9-3, oscillator board No. 1 schematic (serial numbers below 10175) | c. 1978-80 drawing of the board after kit 94-001 | fantasyjackpalance.com `002/903-osc-print-schem-below.gif`, 4422x3078, 300 dpi (sha256 949a0552...); synthfool PDF p. 5 is the same scan at half size | Board 1 netlist |
| S-08001 | Moog Music dwg 08-001, "Mini D oscillator PC brd assy schematic" | 4-10-72 | fantasyjackpalance.com `schematics300/minimoog-schematics-14.gif` (4544x2936) and halves 14a/14b | Board 1 as built (before kit 94-001) |
| S-F97 | Figure 9-7, contour generator and keyboard board No. 2 | c. 1978-80 | `002/907-cont-gen-key-schem.gif` (4572x3084) | Board 2 (transcribed: board2.md) |
| S-F98 | Figure 9-8, power supply board No. 3 | c. 1978-80 | `002/908-power-print-circ-schem.gif` (4470x3084) | Board 3 (to do) |
| S-F911 | Figure 9-11, filter board No. 4 | c. 1978-80 | `002/911-filt-print-circ-schem.gif` (4632x3132) | Board 4 (to do) |
| S-F912 | Figure 9-12, left hand controller | c. 1978-80 | `002/912-left-cont-circ-schem.gif` | |
| S-F916 | Figure 9-16, front panel assembly wiring (later edition: range switches with 1K resistors and the octave buffer board) | c. 1978-80 | `002/916-front-panel-asm.gif` (5988x3131) | Front panel parts, switch wiring |
| S-F917 | Figure 9-17, interconnecting wiring (later edition) | c. 1978-80 | `002/917-interconnect-wiring.gif` (5958x3084) | TUNE and FREQUENCY pot networks, pin assignments |
| S-RAM | R. A. Moog Co. (Trumansburg, N.Y.) circuit drawings: 1436 keyboard circuit, 1437 contour (obsolete after SN 1059), 1444 modulation mix amp, 1445 dual VCA, 1446 filter (revisions to 6/14/71), 1447 headphone amp, 1448 waveform switching, 1449 left hand controller (rev B 7/1/71), A-440, preamp, noise, power supply, block diagram | 1970-71 | fantasyjackpalance.com `schematics300/minimoog-schematics-01..13.gif` (300 dpi); also synthfool PDF pp. 33-46 at half size | Early boards; 1448 waveform switching and 1449 pitch wheel used |
| S-T75 | Table 7-5, old oscillator board parts list (with the synthfool copy's hand note "SG 3821") | c. 1978-80 | synthfool PDF pp. 24-26; fantasyjackpalance `parts150/` | Values cross-check |
| S-T73 | Table 7-3, front panel and left hand controller parts list (pot values and tapers) | c. 1978-80 | synthfool PDF p. 21 | Front panel pots |
| S-SM | Norlin "Service Manual for Minimoog Model 204D", complete: sections 1-9 (circuit description 2.13-2.19 for the old board, adjustments 5.3-5.4, parts lists, modifications 8.1-8.8) | c. 1978-80 | manuals.fdiskc.com / archive.org `moog_Moog_Minimoog_Service_Manual` (82 pp., 4-bit grey) | Circuit descriptions |
| S-T53U | Norlin replacement page, Table 5-3 "Oscillator tuning procedure (serial numbers below 10175)" | c. 1980 | synthfool `updated_tuning_early_vcos.jpg` | Calibration (NorlinUpdated) |
| S-FOLK | R. J. Folkman, "Mini-Moog Field Service Manual" (Moog Music Inc.) | July 1973 | vintagesynthparts.com PDF (24 pp., text) | Period calibration (Folkman1973), kit 94-001, filter modification, pitch wheel detent |
| S-804C | Norlin service bulletin 804C "Minimoog Manual Addendum" | 9/81 | synthfool `factory_service_bulleting_804C1.jpg`, `...804C2.jpg` | Frequency chart per range; kit serial range 1300-5000; R59 = 90.9 ohm |
| S-832 | Norlin service bulletin 832 (mod wheel bleed-through) | 3/83 | synthfool | Modulation (to do) |
| S-ERR | synthfool.com errata page and board pictorials (vcfvca, keyenv, psu), LHC decay switch correction | 2000s (technician) | synthfool `errors/` | Corrections to Figures 9-7, 9-17; early-unit board values; range switch editions |
| S-KEH | B. Kehew, "Minimoog Changes" (2000), and other technician notes (K. Lightner) | 2000s (secondary) | synthfool `minikl/` | Revision history (serial ranges) |

## Component data

| Tag | Document | Used for |
|---|---|---|
| D-3046 | Intersil CA3046 data sheet FN341.5 (May 2001); RCA File No. 341 (1973, in the 1975 RCA Linear IC databook) | `CA3046_NPN` |
| D-3046M | Intersil application note MM9701 (1997), SPICE models for CA3046, CA3086, CA3127 | Cross-check only (IS inconsistent with the data sheet's VBE; RB = RE = 0) |
| D-E402 | Siliconix E400/E401/E402 data sheet, 1977 Siliconix FET Data Book p. 3-81 | `JE402` |
| D-3392 | Central Semiconductor MPS3392-95 data sheet (2017); GE ETR-15G | `Q2N3392` |
| D-4058 | Central Semiconductor 2N4058-2N4062 data sheet (2014) | `Q2N4058` |
| D-4402 | Central Semiconductor 2N4402/2N4403 data sheet (2014) | `Q2N4402` |
| D-741 | Fairchild uA741 data sheet; TI SLOS094 | `ua741` Boyle macromodel |

## Papers

In the manifest under `papers/`: Huovilainen (DAFx 2004); Stilson and Smith (ICMC 1996);
Stinchcombe, "Analysis of the Moog transistor ladder and derivative filters" (2008);
D'Angelo and Valimaki, "Generalized Moog ladder filter" I and II (IEEE TASLP 2014); Holters
and Zolzer (EUSIPCO 2015); Yeh's thesis (2009) and TASLP papers (2010, 2012); Werner et
al. (DAFx 2015) and Werner's thesis (2016); Moog, US patent 3,475,623 (ladder filter, 1969).
Not obtained (paywalled): D'Angelo and Valimaki, ICASSP 2013; Helie, TASLP 18(4) 2010;
Valimaki and Huovilainen, CMJ 2006.
