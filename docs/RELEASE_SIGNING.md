# Desktop builds and release integrity

The current `.github/workflows/release.yml` workflow runs on manual dispatch and builds macOS and Windows desktop packages. It uploads GitHub Actions artifacts; it does not itself publish a GitHub Release or claim trusted operating-system signing.

The checked-in Tauri configuration uses ad-hoc macOS signing (`signingIdentity: "-"`). Ad-hoc integrity verification is not Developer ID signing or Apple notarization. No Windows Authenticode signing step is configured. Follow your organization's policy for installing these packages.

Automatic upstream updates are disabled: `createUpdaterArtifacts` is false and no updater plugin is configured. The retained upstream release helper scripts are not an active updater, signing or publication pipeline. Do not configure an upstream signing key or publish an upstream update feed for this fork.

For a release, record the exact source commit and successful build run, inspect the artifact contents, calculate SHA-256 checksums and verify the downloaded packages against those checksums. Build packages from the public source revision being released. Windows portable execution requires an existing Microsoft WebView2 runtime; it does not install that runtime.

If trusted macOS signing/notarization or Windows signing is added, review the workflow and publisher identity separately, keep signing credentials in protected CI secrets, and verify the resulting signature before describing the release as signed or notarized. Source tests and artifact upload alone do not prove desktop sign-in or cross-computer transfer.
