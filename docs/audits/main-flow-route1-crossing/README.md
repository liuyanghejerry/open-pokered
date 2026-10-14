# m10 Route 1 entry input budget

The fresh main-flow regression fails because its 24-frame DOWN hold stops in
Viridian City at `(20,35)`. It has not entered Route 1. The original Red ROM
also stops there with this input, so this failure does not justify speeding
up the game engine.

Both m10's initial south crossing and its return from healing use 40 held
frames, followed by the same neutral tail. Their explicit Route 1 navigation
and training checks remain.
If it is still in Viridian City after the crossing input, the error now says
entry did not finish; it must not be described as having warped back from a
map that the run never reached.

## Evidence

Two independent fresh runs of the frozen nickname-fix binary failed at this
site, at frames 56,777 and 54,777. Both final states were overworld,
Viridian City `(20,35)`, facing DOWN, idle, no script, no warp fade.

The next fresh run completed the fixed initial crossing and started training.
It then failed after healing because the return leg still used the same
24-frame hold from `(20,33)`. Its final state at frame 56,874 was again
Viridian City `(20,35)`, facing DOWN and idle. The identical input correction
is therefore also applied to that observed return site. This is evidence
for the two crossings, not for completion of m10 or the full main flow.

Separate native probes used that same frozen binary and a controlled CLI warp
at `(20,33)`, then actual UP and DOWN steps to establish the previous stop
direction. Original probes used the unmodified retail Red ROM and actual
buttons from the verified Continue save to approach `(20,33)` from north.
Both input windows therefore start facing DOWN, idle; the original's previous
stop direction is explicitly DOWN (`4`). The original setup contains no RAM
writes. Reference source is `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, ROM
SHA-1 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`, PyBoy 2.8.1.

| Held DOWN frames | Original, four starting phases | Native, two independent repeats |
| --- | --- | --- |
| 24 | Viridian City `(20,35)` | Viridian City `(20,35)` |
| 36 | Route 1 `(10,0)` | Route 1 `(10,0)` |
| 40 | Route 1 `(10,0)` | Route 1 `(10,0)` |

Forty provides margin across starting phases and matches the required map
entry without changing global frame budgets or other unverified sites.
The original tile commits occur at elapsed frames 16–17, 33–34, then the
Route 1 map entry at 50–51. The final connection continues after releasing
the direction; the driver retains its neutral completion frames.

[Compact input/state evidence](border-input-results.json) records all accepted
trials, source/native provenance and both failures. Full private records remain
at the directories named by `.task-tmp/fidelity-39-plus/audit-836-native-path.txt`
and `audit-836-original-path.txt`. Capture scripts and logs are
`audit-836-nav-border-native.py`/`.log` and
`audit-836-nav-border-original.py`/`.log`. An initial exploration with pulsed
one-frame commands and an original setup ending LEFT was rejected for the
matched-input comparison; the table uses the continuous holds and matching
DOWN approach only.

The existing navigation suite passes all 42 tests:
`python3 -m unittest discover -s scripts -p test_playthrough_navigation.py`.
This change affects regression tooling only, so no game-frame screenshot
changes result. It does not establish full rendered movement parity or
completion of the main-flow regression. A new fresh main-flow run remains
required after this commit.

The fresh `audit-838-full-main-flow-retry2` run with both sites fixed reached
level 13 after 44 training battles and multiple successful healing returns.
It then beat Brock: m10 finished at frame 155884 in PewterGym `(4,2)`,
overworld with no active effect. m11 also passed at frame 179853 in Route4
`(11,6)`. The run is continuing through later milestones; this report does
not claim m49 completion. The binary is the frozen nickname-fix binary;
the later PP-Up numeric change on master is verified separately in PR #227.
