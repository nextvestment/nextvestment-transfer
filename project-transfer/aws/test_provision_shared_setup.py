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


class ProvisionSharedSetupTest(unittest.TestCase):
    def test_policy_is_limited_to_existing_transfer_folder(self):
        policy = provision.access_policy()
        self.assertEqual(policy["Statement"][1]["Condition"]["StringLike"]["s3:prefix"], ["ascendasia/", "ascendasia/*"])
        self.assertEqual(policy["Statement"][2]["Resource"],
                         "arn:aws:s3:::nextvestment-ascendasia-transfer-102076848754/ascendasia/*")
        self.assertTrue(all(action.startswith("s3:") for statement in policy["Statement"]
                            for action in ([statement["Action"]] if isinstance(statement["Action"], str) else statement["Action"])))

    def test_writes_private_import_json_without_printing_secret(self):
        calls = []
        secret = "s" * 40

        def fake_aws(_profile, *args):
            calls.append(args)
            if args[:2] == ("sts", "get-caller-identity"):
                return {"Account": provision.ACCOUNT}
            if args[:2] == ("iam", "get-user"):
                raise RuntimeError("NoSuchEntity")
            if args[:2] == ("iam", "create-access-key"):
                return {"AccessKey": {"AccessKeyId": "AKIA" + "A" * 16, "SecretAccessKey": secret}}
            return {}

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "setup.json"
            stdout = io.StringIO()
            with patch.object(provision, "aws", side_effect=fake_aws), \
                    patch.object(sys, "argv", ["provision", "--profile", "admin", "--output", str(output)]), \
                    contextlib.redirect_stdout(stdout):
                self.assertEqual(provision.main(), 0)
            setup = json.loads(output.read_text())
            self.assertEqual(setup["secretAccessKey"], secret)
            self.assertEqual(setup["prefix"], "ascendasia/")
            self.assertNotIn(secret, stdout.getvalue())
            if os.name != "nt":
                self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            self.assertIn(("iam", "put-user-policy", "--user-name", provision.USER,
                           "--policy-name", provision.POLICY, "--policy-document", json.dumps(provision.access_policy())), calls)


if __name__ == "__main__":
    unittest.main()
