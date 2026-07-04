# Seat 0 (Alex) — player experience report

**What I did:** Drew for my only turn. Word choices were bicycle / scarecrow / **rainbow**; I picked rainbow as the most legible with polyline strokes. Built it as six ROYGBIV nested arcs (Python-generated polylines) plus two small gray clouds at the base ends. All three opponents — Priya, Theo, and Sam — guessed it. It was the only word in the whole match that *everyone* got (CASTLE, SAXOPHONE, and GUITAR each got only one guesser).

**What worked well:**
- Pick → draw flow was smooth. `word_length` reported 7, matching RAINBOW, and the drawing timer (~406s) was generous.
- Per-stroke canvas rendering was crisp and immediate: strokes with `color` + `size` produced clean arcs, `canvas_ink` registered right away (~30k), and the shared `canvas.png` rendered perfectly when I looked at it.
- Real-time guess feedback worked. While still drawing I could watch `guessed_count` climb and see "Priya guessed the word!" / "Theo guessed the word!" land in chat — the drawer is *not* playing blind. That felt great.

**Friction / oddities (for the DX journal):**
- **Round labeling is confusing.** Every turn is announced as "Round 1 of 1 — X is drawing," even though four different players each took a drawing turn in sequence. With `total_rounds: 1` but 4 rotations, the label never advances and reads as if the match is stuck on the same round. A turn/rotation counter ("Turn 2 of 4") would communicate progress far better.
- **Drawer scoring feels low / disconnected.** I finished with 100 as the drawer despite *all three* players guessing my word, while the guessers ended 900+. Whether intended or not, "everyone guessed my drawing" producing the lowest score on the board is counter-intuitive — worth a balance look at the drawer reward formula.
- **Cross-turn cadence is easy to fall out of (partly on me).** The per-seat `can_pick/can_draw/can_guess/can_continue` flags are a clean model, but turns cycled fast and I did not re-engage as a guesser on the other three turns. Not a game bug, but a note for the multi-agent playtest harness: an agent needs one persistent poll-act loop spanning *all* turns, not a per-turn wait that ends at its own reveal.

**Was it fun?** Yes. Drawing incrementally and watching guesses arrive in real time was genuinely satisfying, and the canvas felt responsive stroke-to-stroke.
