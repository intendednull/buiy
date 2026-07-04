# Seat 2 (Theo) — player report

**Final: 2nd place, 912 pts** (Priya 924, Sam 294, Alex 100).

## What I did
- **Guessed RAINBOW** — instant read: concentric R/O/Y/G/B/V arcs, 7 letters. Correct first try, +357.
- **Guessed CASTLE** — Priya's three gray columns with crenellated (toothed) tops read as battlements; 6 letters fit. Correct first try, +413.
- **Drew SAXOPHONE** — picked it over unicorn/narwhal as the least-ambiguous silhouette. Drew brass body + U-bend + flared bell + black key buttons, iterating by re-reading canvas.png. Only 1/3 guessers (Priya) got it in time.
- **Guessed GUITAR** — Sam drew *nothing* all turn (canvas stayed empty), so I solved it from the hint pattern `_ _ I _ _ R` alone. Correct first try.

## What worked
- Per-stroke canvas.png updates let me actually *iterate* my drawing visually — draw a few strokes, look, widen the bell, add keys. This is the core loop and it felt good.
- The hint-letter reveal rescued the empty-canvas turn — guessers weren't stranded when a drawer contributed nothing.
- Phase/flag state (can_pick/guess/draw/continue, countdown in real seconds) was clean and unambiguous to poll; pick → draw → reveal → next-turn transitions all fired correctly.

## Bugs / oddities / friction
- **Sam's drawing turn produced zero ink** — host `canvas_ink` stayed 0 for the entire turn. The game degraded gracefully (hints filled in), but a drawer that renders nothing is a notable case worth flagging (agent idling vs. a stroke-registration issue — from my seat I can't tell which).
- **Drawer has no read on guesser progress beyond a count.** During my sax turn I saw `guessed 1/3` but no signal of *why* the other two were stuck (were they close? guessing wildly?). Wrong guesses do surface in broadcast chat as `Name: text` (I saw `Priya: spider` on the guitar turn), but they lagged into my per-seat chat digest — during my own turn I saw none, so as the drawer I was flying blind on whether to keep adding detail.

## Verdict
Fun. The draw-and-guess loop is genuinely engaging — deducing a word from a half-finished picture, and watching guesses land on my own art, both landed. Solid acceptance run.
