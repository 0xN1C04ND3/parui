# parui
[![Crates.io](https://img.shields.io/crates/v/parui)](https://crates.io/crates/parui)

### Simple TUI frontend for [paru](https://github.com/morganamilo/paru) or [yay](https://github.com/Jguer/yay).

### Usage

```
Usage: parui [OPTION]... QUERY
        Search for QUERY in the Arch User Repository.,
        Example:
           parui -p=yay rustup

        Options:
           -p=<PROGRAM>
               Selects program used to search AUR
               Not guaranteed to work well
               Default: paru
           -h
               Print this help and exit
```

### Keybinds

parui adopts vim-like keybinds. Those of select mode can be changed in the
[configuration file](#configuration).

| Key                    | Mode   | Action                                        |
|------------------------|--------|-----------------------------------------------|
| \<Return\>             | Insert | Search for query                              |
| \<C-w\>                | Insert | Removes previous word                         |
| \<C-c\>                | Both   | Exits parui                                   |
| \<Escape\>             | Both   | Switch Modes                                  |
| i, /                   | Select | Enter Insert Mode                             |
| \<Return\>             | Select | Install/upgrade selected packages             |
| \<C-j\>, \<C-Down\>    | Select | Moves info one row down                       |
| \<C-k\>, \<C-Up\>      | Select | Moves info one row up                         |
| h, \<Left\>, \<PgUp\>  | Select | Moves one page back                           |
| j, \<Down\>            | Select | Moves one row down                            |
| k, \<Up\>              | Select | Moves one row up                              |
| l, \<Right\>, \<PgDn\> | Select | Moves one page forwards                       |
| g, \<Home\>            | Select | Go to start                                   |
| G, \<End\>             | Select | Go to end                                     |
| \<Space\>              | Select | Select/deselect package                       |
| c                      | Select | Clear selections                              |
| \<S-R\>                | Select | Remove selected packages                      |
| \<S-U\>                | Select | Upgrade the whole system                      |
| x                      | Select | Ignore/unignore selected packages in upgrades |
| n                      | Select | Edit the note of the current package          |
| \<Escape\>             | Note   | Save the note and go back                     |
| q                      | Select | Exits parui                                   |

### Ignoring packages

`x` toggles the selected packages in `/etc/pacman.d/parui-ignore.conf`, a list of `IgnorePkg`
entries, so that pacman, paru and `<S-U>` skip them when upgrading. Ignored packages are shown
crossed out. The first time, parui offers to include that file from the `[options]` section of
`/etc/pacman.conf`. Both files are written through sudo.

### Notes

`n` opens the notes pane in the bottom right corner to write a note about the current package,
such as why it is held back. `<Escape>` saves it, and an empty note is deleted. Packages with a
note are labelled "Note" in the list. Notes are kept in `$XDG_DATA_HOME/parui/notes`, by default
`~/.local/share/parui/notes`.

### Configuration

parui reads `$XDG_CONFIG_HOME/parui/config`, by default `~/.config/parui/config`. Each line is
`key = value`, lines starting with `#` are comments, and every setting is optional:

```
# Program used to search and install packages. -p= takes precedence.
program = paru

# After installing, removing or upgrading: wait for Enter and reopen parui (ask), reopen right
# away (always), or stay in the terminal like upstream parui (never).
reopen = ask

# Notes pane in the bottom right corner, and its height in rows or as a percentage.
notes = true
notes-height = 33%

# Colors of the package list.
color-upgradable = #ff9e40
color-installed = #8cd85a
color-not-installed = #5ca8ff
color-note-label = #9a9a9a
# Text of the row under the cursor, whose background is the color of its package.
color-cursor-text = #1e1e1e

# Keys of select mode, as comma-separated lists that replace the defaults below. An empty list
# unbinds the action. Keys are characters (`R` is Shift-r), or enter, space, tab, backspace,
# delete, insert, up, down, left, right, home, end, pageup, pagedown, f1 to f12, comma and plus,
# optionally after ctrl+, alt+ or shift+. Escape and ctrl+c cannot be rebound.
key-install = enter
key-remove = R
key-upgrade-all = U
key-ignore = x
key-note = n
key-select = space
key-clear-selection = c
key-search = i, /
key-quit = q
key-down = j, down
key-up = k, up
key-page-down = l, right, pagedown
key-page-up = h, left, pageup
key-top = g, home
key-bottom = G, end
key-info-down = ctrl+j, ctrl+down
key-info-up = ctrl+k, ctrl+up
```

Colors are either fixed, as `#rrggbb`, or taken from the terminal theme, as one of its 16 colors
(`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, or the same with
`bright-` in front), a 256-color palette index such as `208`, or `default` for the terminal's text
color. The defaults are fixed because themes often remap their colors: Monokai Pro, for one,
shows `blue` as orange. An invalid setting makes parui exit with the line at fault.

### Images
![Start Screen](https://user-images.githubusercontent.com/24369412/218350990-96a0f294-9612-4103-b43c-98b7ecfa2428.png)
![Info](https://user-images.githubusercontent.com/24369412/218350962-217da502-b8e3-4b0a-9bd7-bafe4e3c92ed.png)
![Info Scrolling](https://user-images.githubusercontent.com/24369412/218350977-39ed3f30-125d-4217-a01d-5b5b151e7aef.png)
![Selections](https://user-images.githubusercontent.com/24369412/218350983-bf1fee64-c635-46f1-a3a8-fdf0c0ad9190.png)
