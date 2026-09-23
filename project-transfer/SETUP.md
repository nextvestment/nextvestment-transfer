# Shared-folder setup

1. Open the desktop client on each computer. Windows portable execution requires an existing WebView2 runtime; it does not install one.
2. For a project-approved long-term IAM key, open Cloud Profiles and choose **Paste shared setup**. Paste a single JSON string in this format. Replace the example values with the administrator-issued key and approved bucket details; never put the completed string in this repository.

   ```json
   {"type":"nextvestment-transfer-share","version":1,"name":"Project Share","bucket":"YOUR_BUCKET","region":"ap-southeast-1","prefix":"project-share/","accessKeyId":"AKIA_YOUR_20_CHARACTER_ID","secretAccessKey":"YOUR_40_CHARACTER_SECRET_KEY"}
   ```

   The example is a shape illustration, not a usable key. `AKIA` IDs are long-term IAM keys; expiring `ASIA` session credentials cannot be imported. The secret is stored in the operating-system vault. The folder settings are stored separately. Select **Open Project Share** to test the folder. An IAM administrator must create the identity and attach a policy restricted to that bucket and prefix; see [the policy template](aws/README.md). A long-term key has no scheduled SSO expiry, but the administrator can revoke or rotate it at any time.

For individual browser sign-in instead:

1. Create a profile using **Browser sign-in (no AWS CLI)**.
3. Enter the AWS access portal URL supplied by your administrator (`https://your-company.awsapps.com/start`) and the Identity Center region.
4. Open the AWS browser page, compare the displayed device code, and sign in using your own identity.
5. Select an assigned account and role and save the connection. Select that profile if another is active.
6. Enter your bucket name, bucket region and permitted folder prefix in Project Share.
7. Upload a test file, download it on the other computer, and compare its checksum. Repeat in the opposite direction.

Your administrator must provide the appropriate assignment and bucket policy. Bucket settings are separate from sign-in credentials. When an SSO session expires, sign in again in the profile settings. If a team is authorized to use one project key, distribute the completed setup string only through the team's approved private channel and rotate it when membership changes. Anyone with the string can use the same folder permissions until the key is revoked.

Native sign-in supports commercial AWS access portals. It does not bypass company network restrictions or provide GovCloud/China portal support.
