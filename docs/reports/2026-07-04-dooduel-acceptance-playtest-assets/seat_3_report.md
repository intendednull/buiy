# Seat 3 (Sam) — player report

**What I actually did.** I fully played exactly one of four turns. Turn 1 (Alex drawing) I read `canvas.png`, saw an unmistakable ROYGBIV arc with two clouds, guessed "rainbow" — correct on the first try, +227. That loop (look at the doodle, race to type a word) was genuinely fun and the picture was instantly legible. My `commands.jsonl` contains exactly one line all match: that guess.

**My own draw turn (GUITAR) — first-person account.** I idled through it and landed zero strokes. I did not attempt any strokes that failed to register; I submitted nothing at all — no `pick`, no `stroke`. What happened on my side: after turn 1 I set a background condition-poll to wake me when the next turn became actionable. It did fire (it exited during turn 2 with `can_guess:true`, Priya's canvas already at 68k ink), but that completion notification only reached me batched at match end — after all four turns were already over. So from my seat I never received a live "your turn to pick / your turn to draw" signal I could act on, and I lost track of the turn entirely. The host confirms the hint system (revealed letters) carried my turn: two players guessed GUITAR off the word blanks + revealed hints alone, on an empty canvas, and I still got drawer credit (227→294, 3rd place).

**What worked well.**
- Canvas-as-shared-image is excellent: crisp colored strokes, clouds and arcs readable straight from `canvas.png`, zero ambiguity identifying the rainbow.
- Scoring/order felt right (fast correct guess scored well); clean reveal ("The word was RAINBOW") and a scoreboard that re-sorts by score each turn.
- `state.json` seat flags were accurate (`can_guess` flipped true exactly when I could guess); public wrong guesses show in chat (saw Priya's "spider" before she got GUITAR).

**Friction / bugs (specific — for the DX journal).**
1. **Turn hand-off is invisible to a polling agent.** I missed 3 of 4 turns purely because turn-advance events surfaced too late (batched at match end), so the match outran my poll loop. Agent players need a tight, event-driven "your move now" push per phase — not a poll the match can outrun.
2. **A drawer who contributes nothing still yields a normal guessed round.** My GUITAR turn resolved as a complete, scored, "guessed" round on an empty canvas — the hint reveals alone were enough. Worth deciding intentionally: an idle drawer probably shouldn't earn full drawer credit, and hints shouldn't fully substitute for a drawing.
3. Per-phase timers are generous (pick 180s / draw 420s) yet the entire 4-turn match still completed inside a single poll cycle from my seat — turns auto-advance fast when I'm not the active participant.

**Verdict:** The core draw-and-guess loop is genuinely fun and the canvas renders beautifully — but as an agent I only got to actually play 1 of 4 turns; the invisible turn hand-off and my empty-canvas turn resolving on hints alone are the real things to fix.
