# Security policy

Nextvestment Transfer handles cloud credentials and private object-storage access on the local computer.

## Report a vulnerability

Do not post credentials, session tokens or sensitive logs in public issues. Use private GitHub security reporting if enabled, or contact a repository maintainer privately. Include the affected version, operating system, reproduction steps and impact with sensitive values removed.

## Credential storage

Manual and custom S3 secret keys and native IAM Identity Center sign-in tokens use the operating-system credential vault, including Windows Credential Manager. Portable mode does not make credentials portable: each computer must sign in independently. Profile metadata contains connection settings and opaque credential references, not reusable secret values.

The app fails closed when required vault storage or a stored credential is unavailable. It does not fall back to a plaintext `secrets.json` file. Re-enter the credential or sign in again when prompted. Environment and shared AWS profile modes use credentials managed outside the app; protect those sources under your organization's policy.

Native sign-in uses your assigned AWS permissions and does not grant access to accounts or buckets. Do not share cached sessions or distribute credentials with a portable package. Report credential leakage, unsafe storage, broken authentication assumptions, remote code execution and release-integrity problems privately.
