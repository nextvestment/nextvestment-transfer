#!/usr/bin/env python3
"""Create a dedicated S3-folder IAM key and a one-paste Transfer setup file.

Run once with an IAM administrator profile. This script never prints the key.
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ACCOUNT = "102076848754"
BUCKET = "nextvestment-ascendasia-transfer-102076848754"
REGION = "ap-southeast-1"
PREFIX = "ascendasia/"
USER = "nextvestment-transfer-share"
POLICY = "NextvestmentTransferShare"


def aws(profile: str, *args: str) -> dict:
    result = subprocess.run(
        ["aws", "--profile", profile, "--region", REGION, *args, "--output", "json", "--no-cli-pager"],
        capture_output=True,
        text=True,
        env={**os.environ, "AWS_PAGER": ""},
        check=False,
    )
    if result.returncode:
        # AWS errors do not contain access-key secrets; still avoid emitting command output.
        detail = result.stderr.strip().splitlines()[-1] if result.stderr.strip() else "AWS command failed"
        raise RuntimeError(detail)
    return json.loads(result.stdout) if result.stdout.strip() else {}


def access_policy() -> dict:
    bucket_arn = f"arn:aws:s3:::{BUCKET}"
    return {
        "Version": "2012-10-17",
        "Statement": [
            {"Sid": "Location", "Effect": "Allow", "Action": "s3:GetBucketLocation", "Resource": bucket_arn},
            {"Sid": "ListFolder", "Effect": "Allow", "Action": "s3:ListBucket", "Resource": bucket_arn,
             "Condition": {"StringLike": {"s3:prefix": [PREFIX, f"{PREFIX}*"]}}},
            {"Sid": "FolderObjects", "Effect": "Allow",
             "Action": ["s3:GetObject", "s3:PutObject", "s3:AbortMultipartUpload", "s3:ListMultipartUploadParts"],
             "Resource": f"{bucket_arn}/{PREFIX}*"},
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, help="AWS CLI profile with IAM administrator permissions")
    parser.add_argument("--output", required=True, type=Path, help="New private JSON file outside a Git repository")
    args = parser.parse_args()
    output = args.output.expanduser().resolve()
    if output.exists():
        parser.error("Output already exists. Choose a new path; never overwrite a saved key.")
    if not output.parent.is_dir():
        parser.error("Output directory does not exist.")
    if any(parent.joinpath(".git").exists() for parent in [output.parent, *output.parents]):
        parser.error("Choose an output path outside a Git repository.")

    caller = aws(args.profile, "sts", "get-caller-identity")
    if caller.get("Account") != ACCOUNT:
        parser.error(f"Expected AWS account {ACCOUNT}, got {caller.get('Account')}.")
    try:
        aws(args.profile, "iam", "get-user", "--user-name", USER)
    except RuntimeError as error:
        if "NoSuchEntity" not in str(error):
            raise
    else:
        parser.error(f"IAM user {USER} already exists. Inspect it before issuing another key.")

    # Reserve a private output path before making AWS changes.
    fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    created = False
    key_id = None
    policy_attached = False
    try:
        aws(args.profile, "iam", "create-user", "--user-name", USER)
        created = True
        aws(args.profile, "iam", "put-user-policy", "--user-name", USER,
            "--policy-name", POLICY, "--policy-document", json.dumps(access_policy()))
        policy_attached = True
        key = aws(args.profile, "iam", "create-access-key", "--user-name", USER)["AccessKey"]
        key_id = key["AccessKeyId"]
        setup = {
            "type": "nextvestment-transfer-share", "version": 1, "name": "Nextvestment Project Share",
            "bucket": BUCKET, "region": REGION, "prefix": PREFIX,
            "accessKeyId": key_id, "secretAccessKey": key["SecretAccessKey"],
        }
        with os.fdopen(fd, "w", encoding="utf-8") as file:
            fd = -1
            json.dump(setup, file, separators=(",", ":"))
            file.write("\n")
            file.flush()
            os.fsync(file.fileno())
        print(f"Created folder-scoped Transfer setup at {output}. Paste its contents into Transfer; do not commit the file.")
        return 0
    except Exception:
        if fd != -1:
            os.close(fd)
        output.unlink(missing_ok=True)
        # Roll back only resources this invocation actually created.
        if key_id:
            try:
                aws(args.profile, "iam", "delete-access-key", "--user-name", USER, "--access-key-id", key_id)
            except Exception:
                print("A key may have been created. Inspect the IAM user before retrying.", file=sys.stderr)
        if policy_attached:
            try:
                aws(args.profile, "iam", "delete-user-policy", "--user-name", USER, "--policy-name", POLICY)
            except Exception:
                print("Inspect the IAM inline policy before retrying.", file=sys.stderr)
        if created:
            try:
                aws(args.profile, "iam", "delete-user", "--user-name", USER)
            except Exception:
                print("Inspect the IAM user before retrying.", file=sys.stderr)
        raise


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, KeyError) as error:
        print(f"Provisioning stopped: {error}", file=sys.stderr)
        sys.exit(1)
