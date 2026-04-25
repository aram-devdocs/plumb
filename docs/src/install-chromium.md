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
