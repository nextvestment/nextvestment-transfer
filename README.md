# Nextvestment Transfer

MIT-licensed desktop S3 shared-folder client based on [Brows3](https://github.com/rgcsekaraa/brows3).

## Use the app

Open a compatible desktop package. Windows portable execution needs an existing WebView2 runtime. Choose **Browser sign-in (no AWS CLI)**, enter your company's AWS access portal URL and IAM Identity Center region, approve the displayed code through AWS, then select an assigned account and role. No AWS CLI or copied AWS configuration is required for native sign-in.

Configure Project Share with your private bucket, region and folder prefix. Upload through the app on one computer, then refresh and download on another. This is an S3 shared-folder interface, not automatic local-filesystem synchronization.

Native sign-in tokens and manual secret keys use the operating-system vault. Each computer signs in independently. Browser sign-in must be repeated when the SSO session expires. Existing environment/shared-profile/manual credential modes remain available. The manual form accepts key pairs only. The app does not grant AWS permissions or make buckets public.

[Generic setup](project-transfer/SETUP.md) | [Scoped AWS policy and bucket template](project-transfer/aws/README.md)

## Build from source

Install Node.js 22, pnpm 11.2.2, Rust and Tauri's platform prerequisites. Run `pnpm install --frozen-lockfile`, then `pnpm tauri dev` or `pnpm tauri build`. End users of built packages do not need these developer tools or Git.

Tests: `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `cargo test --manifest-path src-tauri/Cargo.toml --locked`.

This source snapshot is prepared for review, not a published or signed release. The included workflow creates Actions artifacts. Upstream automatic updates are disabled. Follow your organization's software policy when running internal builds.

## License

MIT. Retain [LICENSE](LICENSE) and [upstream attribution](README.upstream.md).
