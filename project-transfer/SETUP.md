# Shared-folder setup

1. Open the desktop client on each computer. Windows portable execution requires an existing WebView2 runtime; it does not install one.
2. Create a profile using **Browser sign-in (no AWS CLI)**.
3. Enter the AWS access portal URL supplied by your administrator (`https://your-company.awsapps.com/start`) and the Identity Center region.
4. Open the AWS browser page, compare the displayed device code, and sign in using your own identity.
5. Select an assigned account and role and save the connection. Select that profile if another is active.
6. Enter your bucket name, bucket region and permitted folder prefix in Project Share.
7. Upload a test file, download it on the other computer, and compare its checksum. Repeat in the opposite direction.

Your administrator must provide the appropriate assignment and bucket policy. Bucket settings are separate from sign-in credentials. Do not share cached sessions or access keys. When the SSO session expires, sign in again in the profile settings.

Native sign-in supports commercial AWS access portals. It does not bypass company network restrictions or provide GovCloud/China portal support.
