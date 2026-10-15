# Steam installs

What a fresh Steam install of each game contains, so we can answer questions
about the game and test against it without having the game.

1. `sadx.tsv`: Sonic Adventure DX, app 71250, build 411939.
2. `sa2.tsv`: Sonic Adventure 2, app 213610, build 15280.

Each line is one file: path, size, SHA-1 and its first 8 bytes in hex.
The game files themselves aren't here. They're copyrighted and about 5 GB.

## Use

3. `tests/support/steam_install.rs` reads the lists. `lay_out` rebuilds an
   install in a temp folder with every real file name, size and header. The
   rest of each file is empty space, so it costs no disk.
4. The Steam fixtures in `tests/support/steam_fixture.rs` start from that
   layout, so setup runs against the real folders and names.
5. To check a fact, search the lists. For example, the music and voices of
   the Steam SADX are already ADX, even inside the folders named `WMA`:

   ```sh
   grep -i '^SoundData/' tests/fixtures/steam/sadx.tsv | grep -ic '\.wma	'
   ```

## What it can't tell

6. Anything that needs the real bytes, like applying `patch_steam_inst.dat`.
7. How the game looks, sounds or plays. That still needs the game.

## Update

8. After Steam updates a game, reinstall it (or run **Verify integrity of game
   files**) before any setup touches it.
9. Rebuild the list from the Steam library holding the game:

   ```sh
   sh build-aux/steam-manifest.sh "<library>/steamapps" 71250 > tests/fixtures/steam/sadx.tsv
   sh build-aux/steam-manifest.sh "<library>/steamapps" 213610 > tests/fixtures/steam/sa2.tsv
   ```

10. Update the build numbers above, then run the tests.
