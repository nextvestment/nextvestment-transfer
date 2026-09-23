# Nextvestment Transfer

MIT-licensed desktop S3 shared-folder client based on [Brows3](https://github.com/rgcsekaraa/brows3).

## Use the app

Open a compatible desktop package. Windows portable execution needs an existing WebView2 runtime. For a project-approved long-term IAM key, choose **Paste shared setup** in Cloud Profiles and paste one JSON string containing the folder and key pair. The app saves the secret in the operating-system vault and the non-secret folder settings separately. No AWS browser sign-in is needed on that computer. See [shared-folder setup](project-transfer/SETUP.md) for the format and administrator policy.

Alternatively choose **Browser sign-in (no AWS CLI)**, enter your company's AWS access portal URL and IAM Identity Center region, approve the displayed code through AWS, then select an assigned account and role. No AWS CLI or copied AWS configuration is required for native sign-in.

Configure Project Share with your private bucket, region and folder prefix. Upload through the app on one computer, then refresh and download on another. This is an S3 shared-folder interface, not automatic local-filesystem synchronization.

Native sign-in tokens and manual secret keys use the operating-system vault. Browser sign-in must be repeated when the SSO session expires. An imported IAM key remains usable until it is revoked or rotated; Transfer does not create or extend AWS credentials. Existing environment/shared-profile/manual credential modes remain available. The app does not grant AWS permissions or make buckets public.

[Generic setup](project-transfer/SETUP.md) | [Scoped AWS policy and bucket template](project-transfer/aws/README.md)

## Build from source

Install Node.js 22, pnpm 11.2.2, Rust and Tauri's platform prerequisites. Run `pnpm install --frozen-lockfile`, then `pnpm tauri dev` or `pnpm tauri build`. End users of built packages do not need these developer tools or Git.

Tests: `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `cargo test --manifest-path src-tauri/Cargo.toml --locked`.

This source snapshot is prepared for review, not a published or signed release. The included workflow creates Actions artifacts. Upstream automatic updates are disabled. Follow your organization's software policy when running internal builds.

## License

MIT. Retain [LICENSE](LICENSE) and [upstream attribution](README.upstream.md).
