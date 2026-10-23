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

parui adopts vim-like keybinds.

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
note are marked with ✎ in the list. Notes are kept in `$XDG_DATA_HOME/parui/notes`, by default
`~/.local/share/parui/notes`.

### Images
![Start Screen](https://user-images.githubusercontent.com/24369412/218350990-96a0f294-9612-4103-b43c-98b7ecfa2428.png)
![Info](https://user-images.githubusercontent.com/24369412/218350962-217da502-b8e3-4b0a-9bd7-bafe4e3c92ed.png)
![Info Scrolling](https://user-images.githubusercontent.com/24369412/218350977-39ed3f30-125d-4217-a01d-5b5b151e7aef.png)
![Selections](https://user-images.githubusercontent.com/24369412/218350983-bf1fee64-c635-46f1-a3a8-fdf0c0ad9190.png)
