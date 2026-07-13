# Seat 3 — Ada (naive first-timer) — Cycle 1 report

I played a full 4-player match start to finish, joined by room code, guessed
three words (octopus, cupcake, bicycle — all correct), drew a penguin on my own
turn, and finished 1st on the podium ("Ada wins!", 804 pts). Below: light
findings for the clear visual bugs, then the confusion log, which is the point of
this seat.

## Findings

```yaml
id: C1-S3-01
severity: S3
dimension: visual
screen: in_game
seat: 3 (Ada)
build: e768f8b
turn: cross-turn (seen every drawing/picking phase)
repro:
  - "1. As guesser during Priya's Drawing phase, the on-screen countdown number read 240 and stayed 240 across two forced shots ~12s apart, while the ui.md countdown value ticked 215 -> 157."
  - "2. The instant I guessed correctly (a state change), the on-screen number snapped to the correct value (~113, matching ui.md ~110)."
  - "3. Reproduced again during Theo's Drawing phase: screen read 240 across THREE shots while ui.md read 236 -> 202 -> 160, including while the canvas was actively being drawn on."
  - "4. Reproduced during Sam's Picking phase: screen frozen at 99 across multiple shots while ui.md read 238."
expected: >
  The countdown number a player sees should tick down roughly once per second and
  reflect how much time is actually left, so a player can pace their guessing/drawing.
actual: >
  The on-screen countdown NUMBER freezes at its phase-start value and only jumps to
  the correct value when some *other* event forces a repaint (a correct guess, a new
  chat line, a phase change). The circular ring around it keeps animating, so you see
  a moving ring next to a stuck number. During a quiet drawing phase the number sat
  frozen the entire time — a player watching the screen cannot tell how much time is
  left. The underlying timer (ui.md value) is correct, so this looks like a HUD
  render-invalidation bug, not a timer-logic bug.
evidence:
  - findings/timer-frozen-240-vs-202.png   # screen shows 240, ui.md value was 202, canvas blank/being drawn
  - findings/timer-stale-99-vs-54.png      # screen shows 99, ui.md value was 54
  - "ui.md excerpt (Theo drawing): 'size=44x39 text=\"160\"' while screen.png rendered 240"
confidence: high
known_issue: no
suspected_layer: app
notes: >
  Very reproducible. If the displayed countdown is treated as the player's
  authoritative time feedback, this could arguably be S2 (timer feedback wrong);
  I filed S3 because the underlying game timing still advanced correctly and the
  match completed — only the displayed number is unreliable. This was by far the
  most disorienting thing in the whole match.
```

```yaml
id: C1-S3-02
severity: S4
dimension: visual
screen: in_game
seat: 3 (Ada)
build: e768f8b
turn: cross-turn (turn transitions)
repro:
  - "1. Priya's octopus turn ended; I clicked Continue."
  - "2. Next screen: 'Hang tight! Waiting for Theo to pick a word.' — Priya's octopus was still drawn on the canvas behind the card."
  - "3. It only cleared to blank once Theo actually started drawing."
  - "4. Reproduced: my own penguin stayed on the canvas during Sam's 'Waiting for Sam to pick a word' phase, then cleared when Sam started drawing."
expected: >
  When a turn ends, the canvas should clear before the next drawer's phase, so the
  previous drawing doesn't bleed into the next turn.
actual: >
  The previous drawer's artwork persists on the canvas through the *entire* next
  picking/'Hang tight!' phase and only clears when the new drawer's first stroke
  lands. It shows faintly behind the 'Hang tight!' card.
evidence:
  - findings/canvas-not-cleared-between-turns.png   # octopus still visible during Theo's pick phase
confidence: high
known_issue: no
suspected_layer: app
notes: >
  Cosmetic — it clears once drawing starts and doesn't affect scoring — but as a
  first-timer it made me think 'is the new person going to draw on top of the old
  picture?'
```

```yaml
id: C1-S3-03
severity: S4
dimension: visual
screen: join
seat: 3 (Ada)
build: e768f8b
turn: pre-match
repro:
  - "1. Clicked 'Join a room'; the room-code text field rendered with a dashed rectangle overlapping only the left half of the field, plus a wider grey box behind it (two misaligned boxes)."
  - "2. Same dashed-rectangle-plus-notch artifact appeared around the room-code display box on the 'You're in!' lobby screen."
expected: >
  A clean single input field / code display.
actual: >
  A stray dashed rectangle (with a small dashed notch floating above center) is
  drawn over the code field/display, misaligned with the actual box. It reads as
  a rendering glitch. (If it's meant to be a 'tear-off ticket' style decoration,
  the notch and misalignment make it look broken rather than intentional.)
evidence:
  - findings/dashed-box-roomcode.png   # 'You're in!' lobby, dashed artifact over the VQPQTB code box
confidence: medium
known_issue: no
suspected_layer: app
notes: Minor, but it's the first thing you see when joining, so it sets a "buggy" first impression.
```

## Session report

### What I did
- Started on the Home screen as "Ada". Waited for the host's room code, then joined
  via "Join a room" -> typed the code (VQPQTB) -> "Join room".
- Sat in the lobby ("You're in!") until the host (Priya) started the game.
- Turn 1 (Priya drawing): guessed **octopus** correctly from a partial drawing —
  jumped to 1st with 264.
- Turn 2 (Theo drawing): guessed **cupcake** correctly — 505, still leading.
- Turn 3 (my turn): picked **PENGUIN** from KANGAROO/SANDWICH/PENGUIN and drew a
  full penguin — egg body, belly curve, two eyes, orange beak, two orange feet,
  side flippers. Nobody guessed it (0 of 3) and the turn timed out.
- Turn 4 (Sam drawing): guessed **bicycle** correctly the moment the frame appeared
  between the two wheels — finished at 804.
- Podium: "Ada wins!" — I placed 1st. Wrote this report and quit.

### What worked well
- **The core guess loop is genuinely fun.** Watching a drawing grow and getting to
  guess from a *partial* picture, then seeing "You guessed it!" flip green and my
  score leap up the board — that's the good stuff. Octopus -> instant 1st place was
  a real little hit of delight.
- **"Your turn to draw — Pick a word!"** was the clearest moment in the whole game.
  Three big word buttons, no ambiguity, I knew exactly what to do.
- **Drawing with the real toolbar felt good.** Brush/color/size were obvious, strokes
  landed where I aimed, and switching to orange for the beak/feet just worked. I made
  a recognizable penguin on my first try.
- **The reveal screens are excellent.** "The word was X" plus a per-player +points
  breakdown told me exactly what happened and who scored. Clear and satisfying.
- **The podium landed emotionally.** "Ada wins!" in the hand-drawn font with my taller
  purple pillar in the center — a nice payoff. The hand-drawn aesthetic throughout is
  charming.
- **Wrong guesses in chat are a nice touch** — seeing "Sam: muffin", "Sam: cake",
  "Theo: glasses" made the other players feel alive and human.

### How it felt (the confusion log, in order)
1. **Home screen: which button do I press?** The huge purple "▶ Play" button grabbed
   my eye first, but the small print underneath said it's a "solo demo." To actually
   play with people I had to use the *smaller* "Join a room" button. The most
   prominent button wasn't the one I wanted — mild first hesitation.
2. **Join screen: is that a real code already?** The input showed "7XQ2KP" in grey. For
   a second I thought a code was pre-filled and I should just hit "Join room." It's a
   placeholder, but it looks exactly like a real 6-char code, so I hesitated.
3. **The dashed-box glitch.** On the join field *and* the "You're in!" room-code box
   there's a stray dashed rectangle drawn over the box, misaligned. My literal thought:
   "is that broken?" It made the very first screens feel slightly buggy. (Finding
   C1-S3-03.)
4. **"Wait, where's the 4th player?"** In the lobby the roster showed only Priya, Theo,
   and me — I worried Sam hadn't made it. Sam appeared once the game started, so it was
   just join timing, but for a moment I wasn't sure the match was full.
5. **Lobby -> game was abrupt.** The host started and I was just... in the game. No
   "starting in 3, 2, 1" beat. Minor, but it caught me off guard.
6. **"Round 1 / 1"? Only one round?** As a first-timer that read as "this'll be over
   fast." (It turned out each of the 4 players draws once within the one round, so it
   was a full match — but the label made me expect something shorter.)
7. **Everything went grey and I thought something was wrong.** When it wasn't my turn
   the whole screen dimmed. It took me a second to realize "oh, this means I'm waiting,"
   not "the app froze." Once I got it, it's a fine convention.
8. **The countdown number confused me the most.** The big number by the ring jumped
   from ~120 to ~240 (I think the timer resets bigger for the drawing phase), and then
   it just... sat at 240 and never moved while the ring kept spinning. I genuinely
   could not tell how much time was left. Later it would suddenly snap to a much lower
   number the instant someone guessed. As a player, the timer felt frozen and then
   teleporting — I stopped trusting it entirely. (Finding C1-S3-01, my biggest one.)
9. **The old drawing didn't clear.** Between turns, while waiting for the next person
   to pick, the *previous* drawing was still sitting on the canvas behind the "Hang
   tight!" card (I saw the octopus, then later my own penguin). I thought "is the next
   person going to draw over that?" It cleared once they started, but it was
   confusing. (Finding C1-S3-02.)
10. **"Why does the person drawing have 0 points?"** Mid-turn, Priya (the drawer) sat at
    0 even though two of us had guessed her octopus. I assumed drawers didn't score —
    then at the reveal she got +67. So drawer points are awarded at the reveal, not
    live. Not a bug, just a beat where the scoreboard didn't match my expectation.
11. **I drew a clear penguin and NOBODY guessed it.** 0 of 3. That was the low point of
    the match for me emotionally — I made a genuinely recognizable penguin (beak, feet,
    the works) and just watched the timer (frozen at 240, so I couldn't even tell how
    long the humiliation would last) run out with zero guesses. I suspect the other
    players were just slow rather than anything broken, but as the drawer it felt
    deflating and a little lonely.
12. **Dead time waiting for Sam to pick.** Before Sam's turn, "Waiting for Sam to pick a
    word" sat there for ~45 seconds. Combined with the frozen timer, I had no idea if it
    would ever advance or if the game had hung. That was the most tempted-to-give-up
    moment.
13. **Small thing: stale "is drawing" lines.** The chat/side panel kept old
    "Round 1 of 1 — Priya is drawing" lines around even after the turn moved on, so it
    stacked up a little confusingly.
14. **The podium had no confetti that I could see.** I half-expected a celebration; the
    podium is clean and nice, but a bit quiet for a "you won" moment. (Could be
    animation timing — I only see single frames.)

Overall the *good* moments (guessing, the score-jump drama, drawing my own thing,
winning) clearly outweighed the rough ones, and the confusion was concentrated in two
places: the untrustworthy timer and the dead-time stalls between turns.

### Verdict
**Charming-but-jittery.** The game underneath is fun and the hand-drawn style is
lovely, but a frozen-looking timer and dead-air stalls between turns kept yanking me
out of it — a human would enjoy this match, but would grumble at the clock and the
waiting.

## Harness notes
- Not a game bug, but worth flagging for harness tuning: the three other players
  (all agents) were often very slow — Sam took ~45s+ just to pick a word, and nobody
  guessed my penguin at all (0 of 3). This produced long dead stretches and made my
  own draw turn score 0. If that's agent latency rather than game behavior, it colors
  the "feel" read (the pacing gripes above are partly downstream of it).
- The on-screen screen.png countdown number and the ui.md countdown value were
  frequently far apart in the SAME forced-shot (e.g. 240 vs 202, 99 vs 54, 99 vs 238).
  I treated screen.png as ground truth for a player-facing finding (that's what a
  player sees), but the divergence could partly be screenshot-vs-snapshot capture
  timing in the harness — I couldn't fully separate "HUD render freeze" from "capture
  skew." The frozen-at-240-across-three-shots-while-actively-drawing case is what
  convinced me it's a real render freeze, not just capture skew.
- No driver errors or NotFound issues on my side; every click/set_value/stroke acked
  cleanly. The say.sh ack discipline worked well.
