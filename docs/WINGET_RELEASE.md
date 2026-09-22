# Windows distribution status

Nextvestment Transfer has no Winget publication step in its current workflow. The workflow builds Windows packages and uploads Actions artifacts. Release hosting and any package-manager publication are separate operations.

`Brows3Team.Brows3` belongs to the upstream Brows3 application. Do not submit Nextvestment Transfer packages under that identifier or use the upstream project's release URLs as this fork's downloads. The retained upstream Winget helper scripts are historical tooling and are not invoked by the current build workflow.

If Winget distribution is introduced later, establish a publisher-owned identifier for this application, use the exact verified public installer URLs and SHA-256 digests, review installer metadata, and complete the package catalog's submission/review process. No Winget availability is claimed by this repository.
