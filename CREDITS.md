# Credits

## Projects this work relies on

| Project | Authors | Licence | How it is used |
|---|---|---|---|
| [OpenKH](https://github.com/OpenKH/OpenKh) | OpenKH contributors | Apache-2.0 | Its command-line programs unpack the game's archives and compute animation poses. Used as programs. Its documentation of the file formats (https://openkh.dev/kh2/) is what the Python readers in `tools/kh2` were written from: they are rewritten from scratch, no code copied. |
| [Bevy](https://github.com/bevyengine/bevy) | Bevy contributors | MIT / Apache-2.0 | Game engine used only by the test arena, as a library from crates.io. |
| [AI Game Modding Guides](https://github.com/trevaintdead/ai-game-modding-guides) | trevaintdead and community | MIT | The working method of the project this kit comes from. |

## Documentation consulted (names and descriptions only, no data copied)

- OpenKH: formats of `00battle.bin` and of the animation events.
- KHWiki (https://www.khwiki.com/): the pages Attack, Abilities, Reaction Command and Revenge
  Value, for the names of the abilities and for the effect of Combo Plus, Negative Combo and
  Finishing Plus.

## How the rules were obtained

- Game data tables are read, by the scripts in `tools/kh2`, from the copy of the game of
  whoever runs them. None of it is stored here.
- Frame-by-frame recordings of the game ("telemetry") were used to measure timings, jumps
  and movement, and to check the library.
- Where data and recordings were not enough, the behaviour of the PC version was studied to
  the extent needed for interoperability, and written down as behaviour only, in our own
  words, in `docs/SPEC-FUNZIONALE.md`. No original code is reproduced.
- The library's code was then written from scratch from that document and from the interface
  description, by a separate session that had access to neither the earlier code nor the
  study notes. See `PROVENANCE.md`.

## Tools and AI

Written with the help of an AI coding assistant (Claude Code), directed and play-tested by a
human.

## Game content

No game files are included. Kingdom Hearts belongs to Disney and Square Enix. This is an
unofficial fan project.
