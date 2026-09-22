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

Use individual AWS SSO access where available. For external/VDI use, an AWS administrator should bind `access-policy.example.json` (with the bucket name substituted) to the approved project-only identity or role. Do not distribute a PowerUser session or commit access keys. Temporary credentials require access key, secret key and session token; configure them via an AWS shared profile or environment variables (including AWS_SESSION_TOKEN), not the manual key form. Renew them when they expire. Share only non-secret bucket/region/prefix settings through your approved project channel. Assign each person their own IAM Identity Center access and have each computer sign in independently. Do not share access keys, cached sessions or sign-in tokens.

The sample policy allows listing only `project-share/` and reading/uploading inside that prefix. It intentionally grants no account-wide bucket listing, deletion, IAM administration or application-data access. In the client, use Project Share directly rather than Discover buckets. Deleting, moving or renaming remote files is not supported by this policy; make a corrected upload under a new filename. Version history remains available to the bucket administrator.

For an existing approved bucket, no stack is required: supply its name, region and permitted prefix to the client and grant equivalent scoped access. Do not replace an existing bucket policy blindly.

Before real delivery, upload a non-sensitive test file from one computer, download it on the other and compare its SHA-256. Only then transfer your approved files. VDI network and software policies still apply; do not bypass endpoint restrictions.
