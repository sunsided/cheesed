# Cheesed!

### *Episode One: A Hole New World*

A turn-based block-pushing puzzle in DOS-era dress, built on
[game-toolkit](https://github.com/sunsided/game-toolkit). You're a mouse in an old cellar,
and every mouse hole needs a cheese in it.

- **Arrows** (WASD, HJKL, D-pad, or tap/click towards where you want to go): walk. Walk into
  a cheese to push it. You can push one cheese at a time, and you can never pull.
- **Z** (U, Backspace, pad B): undo - as often as you like. **R** (pad Y): restart (undoable).
- **P** pause, **Esc** back to the level menu, **M** mute, **C** scanlines.

## Cheeses

- **Wedge**: the honest one. One push, one tile.
- **Red wheel**: heavy. It only rolls if you take a run-up: your previous step must go the
  same way you push.
- **Brie**: slippery. It slides until it hits something - straight over empty holes - and
  leaves a greasy streak on the floorboards.

## Cats

Cats pace back and forth, one step per move you make, and turn around when blocked. Paw
prints show where each cat steps next. Cheese blocks cats too. Get caught and it's an undo.

## Crumbs and stars

Every step costs a crumb. Each level has a par - the fewest moves possible, found by the
built-in solver - so three stars are always within reach.

## Shareware

This is Episode One, the shareware episode. Please copy this disk for all your friends.
Episodes Two and Three are available in the registered version, which does not exist.

## Building

```sh
task run          # native
task web          # browser at http://localhost:8080 (needs trunk)
task test         # rules + a solver proving every level is solvable at par
task solve        # print optimal solutions (FILE=my_levels.txt for your own)
task snapshot     # render every screen to snapshots/
```

## License

EUPL-1.2. Dependency notices: `THIRD_PARTY_LICENSES.html`.
