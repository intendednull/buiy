# Seat 1 (Priya) — Player Report

**Result: I won, 924 pts.** Final: Priya 924, Theo 912, Sam 294, Alex 100.

## What I did
- **Guessed RAINBOW** (turn 1, Alex drawing): the canvas rendered a perfect 7-arc ROYGBIV rainbow; 7-letter blank matched instantly. First-read guess, correct.
- **Drew CASTLE** (my turn): built it incrementally — filled three tower/keep bodies, added crenellations, connected them with curtain walls, then a brown arched gate, window slits, and two red flags. Read as a castle so clearly that Theo guessed it while I was still mid-stage-2 (before flags/gate even landed).
- **Guessed SAXOPHONE** (Theo drawing): got it from a *partial* sketch — just a gold U-curve + a small neck loop, ink barely 9k. Gold color + curved-instrument silhouette + 9 letters was enough. Correct on first try.
- **Guessed GUITAR** (Sam drawing): Sam never drew a single stroke (canvas stayed blank the whole turn). I deduced it purely from the letter hints `_ _ I _ _ R`. First tried "spider" (wrong, shown publicly), then "guitar" — correct with ~4s left. That swing put me ahead of Theo for the win.

## What worked well
- **canvas.png as the guesser's eye is excellent.** Per-stroke updates meant I could guess from a *half-finished* drawing (saxophone at ~9k ink), which is exactly how the real game feels. The Read-tool image render was crisp and legible.
- **Word-length blanks + hint letters are load-bearing and correct.** The `_ _ I _ _ R` reveal let me still play a stalled turn — that's the mechanic doing its job.
- **Per-seat views are honest and clear:** my role, blanks, choices when picking, and my private wrong-guess ("Priya: spider") all showed correctly; the drawer never saw the word leaked to guessers.
- **Scoreboard/score deltas felt right** — faster + earlier guesses scored more, drawer got credit when others guessed, and the race stayed tight (12-pt final margin) which made it genuinely tense and fun.

## Bugs / friction (for the DX journal)
1. **Stalled/inactive drawer has no graceful handling.** Sam never drew (ink stayed 0 for the full ~420s draw timer AND ran the 180s pick timer down to 0 first). The turn just burns ~10 minutes of wall-clock with a blank canvas and no drawer-idle detection / skip / "drawer left" fallback. A real game needs an AFK-drawer skip or a much shorter idle timeout.
2. **Reveal auto-advance is inconsistent.** After my *drawing* turn and after Theo's turn, the phase jumped drawing→picking without ever exposing `can_continue=True` to me (the reveal window elapsed on the timer). But after turn 1 and the final turn I *did* get `can_continue`. So sometimes "continue" is offered, sometimes the reveal just times through — the contract for when a seat can continue isn't uniform. Not blocking, but confusing as a player.
3. **Timers are very long for a 4-bot match** (pick 180s, draw 420s). Fine per the charter, but combined with bug #1 it made the back half of the match mostly waiting. A "everyone has guessed → end turn early" already fires for full-guess turns; an equivalent early-out for a non-drawing drawer would help a lot.
4. Minor: the drawn arch on my castle gate rendered closer to a flat-topped rectangle than a rounded arch — my semicircle polyline (size-10 stroke) didn't read as curved at that scale. Cosmetic; polyline rendering itself was accurate.

## Verdict
Genuinely fun and fully playable — I could draw a recognizable castle stroke-by-stroke and guess from live/partial art and from letter hints alone, and the scoring made it a real race. The one real gap is drawer-idle handling (a blank 10-minute turn), plus the uneven reveal→continue timing.
