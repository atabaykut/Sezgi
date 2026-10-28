# Removing Sezgi

Sezgi never edits your application's code. What it leaves behind is listed below; remove the parts you want, in this order.

## 1. Undo the AI tool connections

In each application folder where you ran `sezgi bagla` (`connect`):

```sh
sezgi kaldir              # = sezgi remove; add --deneme (--dry-run) to see what would change
```

It reverts everything `sezgi bagla` added: MCP connection entries and "ask Sezgi first" lines (`--yonlendir`). A file you have not edited since is restored byte for byte; otherwise only Sezgi's entry is removed. `--harita-sil` (`--delete-map`) also deletes the map files; `--evet` (`--yes`) confirms without a terminal.

If you added the Sezgi GitHub Action (`action.yml`) to a workflow, remove that step yourself.

## 2. Delete the map

Delete the `.sezgi` folder in each application (or the folder you passed with `--cikti` / `--out`). `kaldir --harita-sil` removes only the files it knows, so files such as `gozlem.json` or `kaynaksiz.json` may remain; deleting the folder removes everything.

## 3. Remove the optional REA install

```sh
sezgi rea kaldir          # = sezgi rea uninstall; --evet without a terminal
```

This deletes the `rea` folder inside the configuration directory. Step 5 removes it too.

## 4. Remove keychain entries

On Windows and macOS, live mode keeps secrets in the OS keychain. `sezgi oturum kapat` (`session close`) deletes the session entries for the current project; the log key has no removal command. Look for these service names:

- `sezgi-oturum` (the human's session for the app)
- `sezgi-oturum-yenileme` (OIDC refresh token)
- `sezgi-kayit` (the key that encrypts the request log)

The account name is the project's id: the folder name under `uygulamalar/` in the configuration directory.

- **Windows:** Control Panel, Credential Manager, Windows Credentials, generic credentials containing these names.
- **macOS:** Keychain Access, or `security delete-generic-password -s sezgi-kayit -a <project-id>` (repeat per service and project).
- **Linux:** new secrets are stored as owner-only files in the configuration directory, removed in step 5.

Deleting the log key makes the existing log unreadable, which is intended when removing Sezgi.

## 5. Delete the configuration directory

The location is the first of these that is set (`yapilandirma_klasoru` in `src/imza.rs`):

| Platform | Path |
|---|---|
| Windows | `%APPDATA%\sezgi` |
| Linux, macOS | `$XDG_CONFIG_HOME/sezgi`, otherwise `~/.config/sezgi` |

It holds signing keys (`anahtarlar/`), pinned keys, connection backups (`eklemeler.json`, `yedekler/`), live-mode state and logs (`uygulamalar/`), switchboard files and REA. Back up signing keys first if you may sign cards again.

## 6. Delete the system anchor (only if you set one up)

`sezgi insan kur --sistem` wrote a file that needs administrator rights to delete:

- Linux, macOS: `sudo rm /etc/sezgi/insan-<uid>.json` (and the folder if empty)
- Windows, from a terminal run as administrator: `C:\ProgramData\sezgi\insan-<user>.json` and the `C:\ProgramData\sezgi` folder

## 7. Remove the program

```sh
npm uninstall -g sezgi
```

If you built from source, delete the binary and unset `SEZGI_IKILI`.

## Migrating away

Everything Sezgi produces is in plain files. `.sezgi/openapi.json` is a standard OpenAPI 3.1 document that other tools (documentation, client generators, OpenAPI-to-MCP servers) can read. `benlik.json`, `kanit.json` and `graf.json` are plain JSON. `kapilar.json` is an internal format and not stable.
