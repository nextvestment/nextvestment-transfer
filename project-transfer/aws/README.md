# Project exchange infrastructure

Deploy `shared-bucket.yaml` in your approved AWS account after verifying caller identity. Use a dedicated bucket; do not repurpose the application release bucket.

```sh
aws sts get-caller-identity --profile YOUR_PROFILE
aws cloudformation deploy --profile YOUR_PROFILE --region YOUR_BUCKET_REGION \
  --stack-name project-share-project-transfer \
  --template-file project-transfer/aws/shared-bucket.yaml \
  --parameter-overrides BucketName=YOUR_UNIQUE_BUCKET
```

The template retains the bucket on stack deletion, encrypts objects, enables versions, blocks public access and denies non-TLS requests. It creates no IAM identities and no credentials. It does not delete uploaded files automatically. Interrupted multipart uploads expire after seven days.

Use individual AWS SSO access where available. For a project-approved VDI shared setup without browser sign-in, an IAM administrator can create one dedicated IAM user with programmatic access, bind `access-policy.example.json` (with bucket name and folder prefix substituted), and issue its long-term access key once. The template must be reviewed against the actual bucket policy and any KMS encryption policy. Share the completed setup JSON only through the approved private team channel; do not commit it, reuse a PowerUser key, or distribute a temporary SSO session. IAM keys remain usable until revoked or rotated and should be rotated when team access changes. The existing SSO PowerUser role does not authorize IAM user/key creation.

The provisioner also works for a client-owned AWS account. An AACS administrator can run it in AWS CloudShell (or from an administrator machine with `--profile ADMIN_PROFILE`) after selecting the approved account, bucket name, region, and folder prefix. With `--create-bucket`, it deploys the included private, encrypted, versioned S3 bucket stack first. For example, after cloning this public repository in CloudShell:

```sh
python3 project-transfer/aws/provision_shared_setup.py \
  --account 506155053808 \
  --bucket AACS_APPROVED_GLOBALLY_UNIQUE_BUCKET \
  --region AACS_APPROVED_REGION \
  --prefix ilp/ \
  --iam-user ilp-transfer-share \
  --name 'AACS ILP Share' \
  --create-bucket \
  --output "$HOME/aacs-ilp-transfer-setup.json"
```

Replace the bucket and region placeholders with values approved by AACS; do not assume the sandbox region. The script verifies the caller account, refuses an existing IAM user or output file, creates a user allowed only in the specified bucket/prefix, writes the secret to a private file, and never prints it. Review its policy before running. Copy the file contents into **Paste shared setup** on each approved computer. Keep the file out of Git. If IAM creation fails, inspect the IAM user before retrying; the script attempts to roll back its IAM changes and retains any created bucket stack.

For Nextvestment vendor testing, an administrator can use the same script without `--create-bucket`, with `--account 102076848754 --bucket nextvestment-ascendasia-transfer-102076848754 --region ap-southeast-1 --prefix ascendasia/`. The current `nextvestment` PowerUser SSO profile can verify that bucket but is denied IAM user creation. Do not use the vendor bucket for AACS-owned files unless AACS explicitly approves that arrangement.

The sample policy allows listing only `project-share/` and reading/uploading inside that prefix. It intentionally grants no account-wide bucket listing, deletion, IAM administration or application-data access. In the client, use Project Share directly rather than Discover buckets. Deleting, moving or renaming remote files is not supported by this policy; make a corrected upload under a new filename. Version history remains available to the bucket administrator.

For an existing approved bucket, no stack is required: supply its name, region and permitted prefix to the client and grant equivalent scoped access. Do not replace an existing bucket policy blindly.

Before real delivery, upload a non-sensitive test file from one computer, download it on the other and compare its SHA-256. Only then transfer your approved files. VDI network and software policies still apply; do not bypass endpoint restrictions.
