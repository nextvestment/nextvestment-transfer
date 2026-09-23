#!/usr/bin/env python3
"""Create a private shared folder, scoped IAM key and one-paste Transfer setup."""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Optional

POLICY_NAME = "TransferShareFolder"
BUCKET_TEMPLATE = Path(__file__).with_name("shared-bucket.yaml")


def run_aws(profile: Optional[str], region: str, *args: str, json_output: bool = True) -> dict:
    command = ["aws", *(["--profile", profile] if profile else []), "--region", region, *args, "--no-cli-pager"]
    if json_output:
        command.extend(["--output", "json"])
    result = subprocess.run(command, capture_output=True, text=True, check=False,
                            env={**os.environ, "AWS_PAGER": ""})
    if result.returncode:
        detail = result.stderr.strip().splitlines()[-1] if result.stderr.strip() else "AWS command failed"
        raise RuntimeError(detail)
    return json.loads(result.stdout) if json_output and result.stdout.strip() else {}


def access_policy(bucket: str, prefix: str) -> dict:
    arn = f"arn:aws:s3:::{bucket}"
    return {"Version": "2012-10-17", "Statement": [
        {"Sid": "Location", "Effect": "Allow", "Action": "s3:GetBucketLocation", "Resource": arn},
        {"Sid": "ListFolder", "Effect": "Allow", "Action": "s3:ListBucket", "Resource": arn,
         "Condition": {"StringLike": {"s3:prefix": [prefix, f"{prefix}*"]}}},
        {"Sid": "FolderObjects", "Effect": "Allow",
         "Action": ["s3:GetObject", "s3:PutObject", "s3:AbortMultipartUpload", "s3:ListMultipartUploadParts"],
         "Resource": f"{arn}/{prefix}*"},
    ]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", help="AWS CLI administrator profile; omit in AWS CloudShell")
    parser.add_argument("--account", required=True, help="Expected 12-digit AWS account ID")
    parser.add_argument("--bucket", required=True, help="Approved transfer bucket name")
    parser.add_argument("--region", required=True, help="Transfer bucket AWS region")
    parser.add_argument("--prefix", required=True, help="Shared folder prefix, for example ilp/")
    parser.add_argument("--iam-user", default="transfer-share", help="New dedicated IAM user name")
    parser.add_argument("--name", default="Project Share", help="Profile name shown in Transfer")
    parser.add_argument("--create-bucket", action="store_true", help="Deploy the private versioned bucket stack")
    parser.add_argument("--output", required=True, type=Path, help="New private JSON file outside Git")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]{12}", args.account):
        parser.error("--account must be a 12-digit AWS account ID.")
    if not re.fullmatch(r"[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]", args.bucket) or ".." in args.bucket:
        parser.error("--bucket must be a valid S3 bucket name.")
    if not re.fullmatch(r"[a-z]{2}(?:-[a-z]+)+-[0-9]+", args.region):
        parser.error("--region must be an AWS region such as ap-southeast-1.")
    prefix = args.prefix.strip("/") + "/"
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]*/", prefix) or ".." in prefix.split("/"):
        parser.error("--prefix must be a nonempty forward-slash folder path.")
    if not re.fullmatch(r"[A-Za-z0-9+=,.@_-]{1,64}", args.iam_user):
        parser.error("--iam-user is not a valid IAM user name.")
    if not args.name.strip() or len(args.name) > 80 or any(ord(char) < 32 for char in args.name):
        parser.error("--name must be a short printable profile name.")
    output = args.output.expanduser().resolve()
    if output.exists() or not output.parent.is_dir():
        parser.error("Choose a new output filename in an existing directory.")
    if any(parent.joinpath(".git").exists() for parent in [output.parent, *output.parents]):
        parser.error("Choose an output path outside a Git repository.")

    aws = lambda *command: run_aws(args.profile, args.region, *command)
    caller = aws("sts", "get-caller-identity")
    if caller.get("Account") != args.account:
        parser.error(f"Expected AWS account {args.account}, got {caller.get('Account')}.")
    try:
        aws("iam", "get-user", "--user-name", args.iam_user)
    except RuntimeError as error:
        if "NoSuchEntity" not in str(error):
            raise
    else:
        parser.error(f"IAM user {args.iam_user} already exists. Inspect it before issuing another key.")

    stack_name = f"transfer-share-{args.account}"
    if args.create_bucket:
        try:
            stack = aws("cloudformation", "describe-stacks", "--stack-name", stack_name)["Stacks"][0]
        except RuntimeError as error:
            if "does not exist" not in str(error):
                raise
        else:
            parameters = {item["ParameterKey"]: item.get("ParameterValue") for item in stack.get("Parameters", [])}
            if parameters.get("BucketName") != args.bucket or parameters.get("FolderPrefix") != prefix:
                raise RuntimeError("Existing transfer bucket stack has different bucket or folder parameters.")

    fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    created = False
    policy_attached = False
    key_id = None
    try:
        if args.create_bucket:
            run_aws(args.profile, args.region, "cloudformation", "deploy", "--stack-name",
                    stack_name, "--template-file", str(BUCKET_TEMPLATE),
                    "--parameter-overrides", f"BucketName={args.bucket}", f"FolderPrefix={prefix}",
                    "--no-fail-on-empty-changeset",
                    json_output=False)
        location = aws("s3api", "get-bucket-location", "--bucket", args.bucket,
                       "--expected-bucket-owner", args.account)
        actual_region = location.get("LocationConstraint") or "us-east-1"
        if actual_region != args.region:
            raise RuntimeError(f"Bucket is in {actual_region}, not {args.region}.")
        aws("iam", "create-user", "--user-name", args.iam_user)
        created = True
        aws("iam", "put-user-policy", "--user-name", args.iam_user, "--policy-name", POLICY_NAME,
            "--policy-document", json.dumps(access_policy(args.bucket, prefix)))
        policy_attached = True
        key = aws("iam", "create-access-key", "--user-name", args.iam_user)["AccessKey"]
        key_id = key["AccessKeyId"]
        setup = {"type": "nextvestment-transfer-share", "version": 1, "name": args.name.strip(),
                 "bucket": args.bucket, "region": args.region, "prefix": prefix,
                 "accessKeyId": key_id, "secretAccessKey": key["SecretAccessKey"]}
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
        # Do not delete a bucket stack: it may contain files. Roll back only IAM resources made here.
        if key_id:
            try:
                aws("iam", "delete-access-key", "--user-name", args.iam_user, "--access-key-id", key_id)
            except Exception:
                print("A key may have been created. Inspect the IAM user before retrying.", file=sys.stderr)
        if policy_attached:
            try:
                aws("iam", "delete-user-policy", "--user-name", args.iam_user, "--policy-name", POLICY_NAME)
            except Exception:
                print("Inspect the IAM inline policy before retrying.", file=sys.stderr)
        if created:
            try:
                aws("iam", "delete-user", "--user-name", args.iam_user)
            except Exception:
                print("Inspect the IAM user before retrying.", file=sys.stderr)
        raise


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, KeyError) as error:
        print(f"Provisioning stopped: {error}", file=sys.stderr)
        sys.exit(1)
