# Rhizome Troubleshooting

Start with `rhizome status`. Most failures come from backend selection, a missing or unhealthy LSP server, or a config file that Rhizome never loaded.

## Fast Triage

| Symptom | First Command | What it usually means |
|---------|---------------|-----------------------|
| Tool returns empty or weak results | `rhizome status` | Tree-sitter is active, but the language needs LSP or better query coverage |
| LSP-required tool says the server is unavailable | `rhizome status` | The server binary is missing, unhealthy, or auto-install is disabled |
| Auto-install does nothing | `printenv RHIZOME_DISABLE_LSP_DOWNLOAD` | Auto-install was disabled or the package manager is missing |
| Config changes have no effect | `RHIZOME_LOG=debug rhizome serve 2>&1 | grep -i config` | The wrong config file is being edited or the project root is not what you expect |
| Export to Hyphae fails | `command -v hyphae` | Hyphae is missing, unreachable, or misconfigured |

## Backend and LSP Issues

### Tool returns empty results even though the file clearly has symbols

**Symptom:** Rhizome returns little or no useful structure for a file that obviously contains code.

**Diagnosis:** Rhizome is using tree-sitter for that language, but the language either lacks a precise query pattern or needs LSP-backed analysis for the result you want.

**Fix:**

1. Check the active backend state:
   ```bash
   rhizome status
   ```
   If LSP is unavailable, Rhizome will stay on tree-sitter or fallback extraction.

2. Try an LSP-backed workflow if the language supports it:
   ```bash
   rhizome status
   ```
   Tools such as rename and reference-heavy operations need LSP to be fully useful.

3. If the language only has weak tree-sitter coverage today, use [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md) to get the server working or add query coverage.

### LSP-required tool says the server is unavailable

**Symptom:** A tool such as rename or richer reference analysis fails with an LSP-unavailable error.

**Diagnosis:** The tool requires LSP, but the server binary is missing, unhealthy, or disabled by config.

**Fix:**

1. Check the resolved server and availability:
   ```bash
   rhizome status
   ```

2. Follow the setup path for that language:
   ```bash
   rhizome status
   ```
   Then use [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md#path-2-lsp-languages-auto-install) for the install or manual override path.

3. Restart Rhizome after installing or changing config:
   ```bash
   rhizome serve
   ```

### Auto-install fails or does nothing

**Symptom:** Rhizome still cannot find the server after trying the auto-install path.

**Diagnosis:** Auto-install is disabled, the package manager is missing from `PATH`, or the install recipe failed.

**Fix:**

1. Check whether auto-install is disabled:
   ```bash
   printenv RHIZOME_DISABLE_LSP_DOWNLOAD
   ```
   Healthy is unset.

2. Check that the package manager exists:
   ```bash
   command -v rustup
   command -v pip3
   command -v npm
   command -v go
   command -v gem
   ```

3. Install the server manually if needed, then verify:
   ```bash
   rustup component add rust-analyzer
   rhizome status
   ```

## Tool and Export Failures

### Tool says the file was not found

**Symptom:** Rhizome reports a file-not-found error for a path that exists in your project.

**Diagnosis:** The command received a relative path, but Rhizome expects an absolute path.

**Fix:**

1. Re-run the command with an absolute path:
   ```bash
   rhizome symbols /absolute/path/to/file.rs
   ```

2. If you are calling Rhizome through MCP, make sure the client is passing an absolute path too.

3. If one file works and another does not, validate the failing file itself:
   ```bash
   file /absolute/path/to/file.rs
   ```

### Tool works on one file and fails on another

**Symptom:** One file behaves normally, but another file in the same repo fails or hangs.

**Diagnosis:** The failing file likely has syntax, encoding, or language-feature issues that stress the selected backend.

**Fix:**

1. Validate the file with a language-native checker:
   ```bash
   rustc --crate-type lib src/file.rs
   python -m py_compile src/file.py
   ```

2. Check encoding:
   ```bash
   file src/file.rs
   ```
   Healthy output is plain ASCII or UTF-8 text.

3. If the file is valid but Rhizome still fails, capture logs and file a bug:
   ```bash
   RHIZOME_LOG=debug rhizome serve
   ```

### Export to Hyphae fails

**Symptom:** Exporting symbols or code graphs to Hyphae fails.

**Diagnosis:** Hyphae is missing, not running, or not reachable through the configured path.

**Fix:**

1. Confirm Hyphae is available:
   ```bash
   command -v hyphae
   ```

2. Start or verify the Hyphae server path you expect to use:
   ```bash
   hyphae serve
   ```

3. Retry export after checking Rhizome logs:
   ```bash
   RHIZOME_LOG=debug rhizome serve
   ```

## Configuration Issues

### Config file is ignored

**Symptom:** Changing config values has no visible effect.

**Diagnosis:** You are editing the wrong file, the TOML is invalid, or Rhizome detected a different project root than you expected.

**Fix:**

1. Check which config files are being loaded:
   ```bash
   RHIZOME_LOG=debug rhizome serve 2>&1 | grep -i config
   ```

2. Validate the TOML you edited:
   ```bash
   python3 -c "import tomllib; tomllib.loads(open('config.toml').read())"
   ```

3. Remember the load order:
   ```text
   1. Global: macOS ~/Library/Application Support/rhizome/config.toml
   2. Global: Linux ${XDG_CONFIG_HOME:-~/.config}/rhizome/config.toml
   3. Global: Windows %APPDATA%\rhizome\config.toml
   4. Project: <project>/.rhizome/config.toml
   5. Environment: RHIZOME_* variables
   ```

### Custom server binary is not being used

**Symptom:** Rhizome keeps launching the default server even though you set a custom one.

**Diagnosis:** The config key is wrong, the config file was not loaded, or the custom binary path itself is invalid.

**Fix:**

1. Verify the language config uses the right key:
   ```toml
   [languages.rust]
   server_binary = "/absolute/path/to/rust-analyzer"
   ```

2. Confirm the binary itself works:
   ```bash
   "/absolute/path/to/rust-analyzer" --version
   ```

3. Restart Rhizome and inspect config loading:
   ```bash
   RHIZOME_LOG=debug rhizome serve 2>&1 | grep -i config
   ```

### Project config does not override global config

**Symptom:** A project-specific `.rhizome/config.toml` seems to be ignored.

**Diagnosis:** Rhizome detected a different project root than you expected, so it never looked at the project config file you edited.

**Fix:**

1. Check the detected project root:
   ```bash
   RHIZOME_LOG=debug rhizome serve 2>&1 | grep -i "project.root"
   ```

2. Confirm the project config is in the detected root:
   ```bash
   cat /path/to/project/.rhizome/config.toml
   ```

3. If the root is wrong, fix the project markers using [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md).

## Performance Issues

### Symbol extraction is slow on large files

**Symptom:** Symbol extraction or structural queries get noticeably slower on very large files.

**Diagnosis:** Large files push more work through tree-sitter or LSP and can hit backend-specific overhead.

**Fix:**

1. Check the file size first:
   ```bash
   wc -l src/file.rs
   du -h src/file.rs
   ```
   Once files get very large, slower extraction is expected.

2. Use an LSP-backed path when available, or narrow the query scope:
   ```bash
   rhizome status
   ```

3. Watch resource usage if performance seems far worse than expected:
   ```bash
   top -p $(pgrep rhizome)
   ```

### MCP server connection feels slow

**Symptom:** Single MCP calls feel slower than expected even on small inputs.

**Diagnosis:** The overhead may be in JSON-RPC transport, a remote hop, or repeated one-off calls that would be faster from the CLI.

**Fix:**

1. Compare with the direct CLI:
   ```bash
   rhizome symbols /absolute/path/to/file.rs
   ```

2. Batch related work where possible instead of making many tiny calls.

3. Keep the server local if you can. Network latency adds up quickly on short code-intelligence calls.

## Error Message Quick Reference

| Error | Cause | Fix |
|-------|-------|-----|
| `"Tool requires LSP but server unavailable"` | The tool needs LSP and the server is missing or unhealthy | Run `rhizome status`, then fix the language server |
| `"LSP auto-install disabled"` | `RHIZOME_DISABLE_LSP_DOWNLOAD` is set or config disabled downloads | Unset the variable or re-enable downloads in config |
| `"Package manager not found: rustup"` | Rhizome cannot run the install recipe for that language | Install the package manager or install the server manually |
| `"File not found: /path/to/file"` | A relative path was passed where Rhizome expects an absolute path | Re-run with an absolute path |
| `"Export failed: Hyphae unavailable"` | Hyphae is missing, not running, or not reachable | Check `command -v hyphae`, then start or repair Hyphae |

## Diagnostic Commands

**Enable debug logging:**
```bash
# All debug output
RHIZOME_LOG=debug rhizome serve

# Rhizome-only logs
RHIZOME_LOG=rhizome=debug rhizome serve

# Tool-module logs
RHIZOME_LOG=rhizome_mcp::tools=debug rhizome serve
```

**Check version:**
```bash
rhizome --version
```

**Inspect current configuration:**
```bash
cat ~/Library/Application\ Support/rhizome/config.toml
cat .rhizome/config.toml
```

**Check state and health:**
```bash
rhizome status
rhizome --help
```

## See also

- [LANGUAGE-SETUP.md](./LANGUAGE-SETUP.md)
- [CONFIG.md](./CONFIG.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
