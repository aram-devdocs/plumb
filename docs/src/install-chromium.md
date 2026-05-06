# Install Chromium

Plumb drives Chrome or Chromium through the Chrome DevTools Protocol. The
browser is not bundled with the `plumb` binary.

Plumb supports Chromium major versions 131 through 150 inclusive. If the
detected browser reports a major version outside that range, `plumb lint`
exits with an unsupported Chromium error instead of producing lint
output.

## macOS

Install Chrome or Chromium:

```bash
brew install --cask google-chrome
```

Plumb checks common app locations such as:

```text
/Applications/Google Chrome.app/Contents/MacOS/Google Chrome
/Applications/Chromium.app/Contents/MacOS/Chromium
```

To use a specific binary:

```bash
plumb lint https://example.com --executable-path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
```

## Linux

Install Chromium from your distribution packages:

```bash
sudo apt-get update
sudo apt-get install chromium
```

Package names vary by distribution. On Debian or Ubuntu systems the binary
is usually `chromium`, `chromium-browser`, or `google-chrome-stable`.

To use a specific binary:

```bash
plumb lint https://example.com --executable-path /usr/bin/chromium
```

## Windows

Install Chrome from the official installer, or install Chromium with a package
manager you already use. Plumb checks the standard Chrome app registration and
common install paths.

To use a specific binary:

```powershell
plumb lint https://example.com --executable-path "C:\Program Files\Google\Chrome\Application\chrome.exe"
```

## Check the version

Run the browser directly to confirm its major version:

```bash
chromium --version
```

The first number in the version must fall in the supported range
(`131` through `150` inclusive). If you have several Chrome or Chromium
builds installed, pass `--executable-path` to select one whose major
version falls in that range.

## Auto-fetch (opt-in)

If you do not have a system Chromium installed, pass
`--auto-fetch-chromium` and Plumb downloads Chrome-for-Testing into a
managed cache directory before the first lint run. Subsequent runs
reuse the cached binary.

```bash
plumb lint https://example.com --auto-fetch-chromium
```

The cache directory follows the platform convention:

| Platform | Cache directory |
|----------|-----------------|
| Linux | `$XDG_CACHE_HOME/plumb/chromium`, falling back to `~/.cache/plumb/chromium` |
| macOS | `~/Library/Caches/plumb/chromium` |
| Windows | `%LOCALAPPDATA%\plumb\chromium` |

After the first install, Plumb writes a `.plumb-sha256` file alongside
the executable. Every subsequent run re-hashes the binary and refuses
to launch on a mismatch — the cache is pinned against accidental or
malicious tampering.

### Trust model

Auto-fetch downloads and executes a third-party binary. Chromium is
served by Google over HTTPS but Plumb does not verify the upstream
publisher signature, so passing `--auto-fetch-chromium` is your
explicit acknowledgement of trust. The SHA-256 sidecar protects against
post-install tampering, not against a compromised upstream. If the
trust model is unacceptable for your environment, install Chromium
yourself and pass `--executable-path` instead.
