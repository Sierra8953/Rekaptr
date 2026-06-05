# Releasing Rekaptr

## Cutting a new version

Every release produces two artifacts that get uploaded to a GitHub Release:

- `rekaptr-setup-<version>.exe` — the actual installer (Inno Setup, ~73 MB)
- `rekaptr-installer.ps1` — a thin bootstrap that downloads the setup.exe; this is what `irm | iex` and the in-app updater both run

A third optional artifact for portable users:

- `rekaptr-x86_64-pc-windows-msvc.zip` — extract-and-run, no installer needed

### Steps

1. **Bump the version** in `Cargo.toml`:
   ```toml
   version = "0.1.1"
   ```
   The "Version X.Y.Z" text on the Settings → About page reads from `CARGO_PKG_VERSION` automatically; no other code edits needed.

2. **Refresh `runtime/` if GStreamer or libmpv changed.** Skip this step otherwise.
   ```powershell
   robocopy C:\Users\user\Desktop\dist runtime /E /XF rekaptr.exe rekaptr.d rekaptr.db
   ```
   If you added or removed top-level DLLs, also update the `include = [...]` list in `dist-workspace.toml` so the portable zip matches.

   **`runtime/` must contain both `ffmpeg.exe` and `ffprobe.exe`** (from the same
   FFmpeg build). The app shells out to ffprobe to compute the cross-session
   `decode-time-offset` — without it, successive recording sessions reset their
   timestamps and playback jumps back to the start at the session seam. They ship
   next to the exe (`{app}\ffmpeg.exe`, `{app}\ffprobe.exe`) and are discovered
   there at runtime. `scripts/build-release.ps1` fails fast if either is missing.

3. **Build the release artifacts:**
   ```powershell
   pwsh scripts/build-release.ps1
   ```
   This runs `dist build` (which produces the portable zip) and then `iscc installer.iss` (which produces the setup.exe), and renders `rekaptr-installer.ps1` from its template with the version baked in.

   Outputs:
   - `target/distrib/rekaptr-x86_64-pc-windows-msvc.zip`
   - `target/installer/rekaptr-setup-<version>.exe`
   - `target/installer/rekaptr-installer.ps1`

4. **Smoke-test locally.** Uninstall the previous version first (Add/Remove Programs → Rekaptr → Uninstall), then run the new installer:
   ```powershell
   Start-Process target\installer\rekaptr-setup-<version>.exe
   ```
   Launch `C:\Program Files\Rekaptr\rekaptr.exe` and confirm it boots.

5. **Commit and push:**
   ```powershell
   git add Cargo.toml Cargo.lock        # plus anything else you changed
   git commit -m "release: v<version>"
   git push
   ```

6. **Create the GitHub Release:**
   - Go to https://github.com/Sierra8953/Rekaptr/releases/new
   - **Tag:** `v<version>` (with the `v` — the installer URLs hardcode this format)
   - **Title:** `v<version>`
   - **Notes:** changelog
   - **Drag in:**
     - `target/installer/rekaptr-setup-<version>.exe`
     - `target/installer/rekaptr-installer.ps1`
     - `target/distrib/rekaptr-x86_64-pc-windows-msvc.zip` *(optional, only if you want to offer portable downloads)*
   - Click **Publish release** (not "Save draft" — drafts aren't reachable by unauthenticated downloaders).

7. **Verify** the asset URLs return 200:
   ```powershell
   "v<version>" | % {
     $tag = $_
     "https://github.com/Sierra8953/Rekaptr/releases/download/$tag/rekaptr-installer.ps1",
     "https://github.com/Sierra8953/Rekaptr/releases/download/$tag/rekaptr-setup-$($tag.TrimStart('v')).exe" |
       % { (Invoke-WebRequest $_ -Method Head -ErrorAction SilentlyContinue).StatusCode, $_ -join '  ' }
   }
   ```

That's it. Users on the previous version will see the new release in **Settings → About → Check for updates**, and clicking **Install update** will run the new `rekaptr-installer.ps1` which fetches the new `setup.exe`.

### Install command for end users

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Sierra8953/Rekaptr/releases/download/v<version>/rekaptr-installer.ps1 | iex"
```

Or, for "always latest":

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Sierra8953/Rekaptr/releases/latest/download/rekaptr-installer.ps1 | iex"
```

The `latest/download/` form requires the asset name to be stable, which `rekaptr-installer.ps1` is.

---

## Portable version

The `rekaptr-x86_64-pc-windows-msvc.zip` produced by `dist build` is a complete portable build. Users:

1. Download the zip from the release page.
2. Extract anywhere (e.g. `D:\Apps\Rekaptr\`).
3. Run `rekaptr.exe`.

No admin needed, no Program Files entry, no Start Menu shortcut, no PATH modification.

### Caveats for portable users

- **No in-app updates.** The updater reads the install receipt at `%LOCALAPPDATA%\rekaptr\rekaptr-receipt.json`, which is only written by the setup.exe. Portable extracts have no receipt, so the Settings → About → Updates section shows "Portable build — reinstall via the official installer to enable updates." disabled.
- **No uninstaller.** Delete the folder when done.
- **App data still goes to `%LOCALAPPDATA%\rekaptr\`** (logs, gst-registry, recordings index) — the portable zip only affects where the binary lives, not where runtime state goes.

### Building portable-only (skip the installer)

If you only want the zip and not the setup.exe:

```powershell
dist build
```

The zip lands at `target/distrib/rekaptr-x86_64-pc-windows-msvc.zip`.
