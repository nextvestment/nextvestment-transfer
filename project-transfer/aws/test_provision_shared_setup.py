import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import provision_shared_setup as provision

ACCOUNT = "506155053808"
BUCKET = "example-ilp-transfer-506155053808"
REGION = "ap-southeast-1"


class ProvisionSharedSetupTest(unittest.TestCase):
    def test_policy_is_limited_to_supplied_bucket_and_folder(self):
        policy = provision.access_policy(BUCKET, "ilp/")
        self.assertEqual(policy["Statement"][1]["Condition"]["StringLike"]["s3:prefix"], ["ilp/", "ilp/*"])
        self.assertEqual(policy["Statement"][2]["Resource"], f"arn:aws:s3:::{BUCKET}/ilp/*")
        self.assertTrue(all(action.startswith("s3:") for statement in policy["Statement"]
                            for action in ([statement["Action"]] if isinstance(statement["Action"], str) else statement["Action"])))

    def test_writes_private_import_json_without_printing_secret(self):
        calls = []
        secret = "s" * 40

        def fake_aws(_profile, _region, *args, **_kwargs):
            calls.append(args)
            if args[:2] == ("sts", "get-caller-identity"):
                return {"Account": ACCOUNT}
            if args[:2] == ("iam", "get-user"):
                raise RuntimeError("NoSuchEntity")
            if args[:2] == ("cloudformation", "describe-stacks"):
                raise RuntimeError("Stack with id transfer-share does not exist")
            if args[:2] == ("s3api", "get-bucket-location"):
                return {"LocationConstraint": REGION}
            if args[:2] == ("iam", "create-access-key"):
                return {"AccessKey": {"AccessKeyId": "AKIA" + "A" * 16, "SecretAccessKey": secret}}
            return {}

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "setup.json"
            stdout = io.StringIO()
            args = ["provision", "--profile", "admin", "--account", ACCOUNT, "--bucket", BUCKET,
                    "--region", REGION, "--prefix", "ilp/", "--create-bucket", "--output", str(output)]
            with patch.object(provision, "run_aws", side_effect=fake_aws), patch.object(sys, "argv", args), \
                    contextlib.redirect_stdout(stdout):
                self.assertEqual(provision.main(), 0)
            setup = json.loads(output.read_text())
            self.assertEqual(setup["secretAccessKey"], secret)
            self.assertEqual(setup["bucket"], BUCKET)
            self.assertEqual(setup["prefix"], "ilp/")
            self.assertNotIn(secret, stdout.getvalue())
            if os.name != "nt":
                self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            self.assertTrue(any(call[:2] == ("cloudformation", "deploy") for call in calls))
            self.assertIn(("iam", "put-user-policy", "--user-name", "transfer-share", "--policy-name",
                           provision.POLICY_NAME, "--policy-document", json.dumps(provision.access_policy(BUCKET, "ilp/"))), calls)

    def test_wrong_account_stops_before_any_write(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "setup.json"
            args = ["provision", "--profile", "admin", "--account", ACCOUNT, "--bucket", BUCKET,
                    "--region", REGION, "--prefix", "ilp/", "--output", str(output)]
            with patch.object(provision, "run_aws", return_value={"Account": "102076848754"}) as aws, \
                    patch.object(sys, "argv", args), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    provision.main()
            self.assertEqual(aws.call_count, 1)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
