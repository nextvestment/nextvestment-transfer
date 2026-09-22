'use client';

import { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, CircularProgress, MenuItem, TextField, Typography } from '@mui/material';
import { identityCenterApi, IdentityCenterAccount, IdentityCenterLogin, IdentityCenterRole, Profile } from '@/lib/tauri';

type Props = { name: string; region: string; profile?: Profile | null; onSaved: (profile: Profile) => Promise<void> };
const message = (error: unknown) => error instanceof Error ? error.message : String(error);

/** Only public device codes and account metadata cross the native boundary. */
export default function IdentityCenterSetup({ name, region, profile, onSaved }: Props) {
  const existing = profile?.credential_type.type === 'IdentityCenter' ? profile.credential_type : undefined;
  const [startUrl, setStartUrl] = useState(existing?.start_url || '');
  const [ssoRegion, setSsoRegion] = useState(existing?.sso_region || 'ap-southeast-1');
  const [login, setLogin] = useState<IdentityCenterLogin | null>(null);
  const [accounts, setAccounts] = useState<IdentityCenterAccount[] | null>(null);
  const [accountId, setAccountId] = useState('');
  const [roles, setRoles] = useState<IdentityCenterRole[]>([]);
  const [role, setRole] = useState('');
  const [busy, setBusy] = useState(false);
  const [loadingRoles, setLoadingRoles] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const generation = useRef(0);
  const session = useRef<string | null>(null);
  const roleGeneration = useRef(0);

  useEffect(() => () => {
    generation.current++;
    roleGeneration.current++;
    if (session.current) void identityCenterApi.cancel(session.current).catch(() => {});
    session.current = null;
  }, []);

  const cancel = () => {
    generation.current++;
    roleGeneration.current++;
    if (session.current) void identityCenterApi.cancel(session.current).catch(() => {});
    session.current = null;
    setLogin(null); setAccounts(null); setAccountId(''); setRoles([]); setRole(''); setBusy(false); setLoadingRoles(false);
  };

  // Recursive timers ensure a slow request never causes overlapping token polls.
  useEffect(() => {
    if (!login || accounts !== null) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const request = generation.current;
    const expiresAt = Date.now() + Math.min(login.expires_in, 1800) * 1000;
    const fail = (error: unknown) => {
      if (disposed || request !== generation.current) return;
      cancel(); setError(message(error));
    };
    const poll = async () => {
      if (disposed || request !== generation.current) return;
      if (Date.now() >= expiresAt) { fail('The browser sign-in expired. Start again to receive a new code.'); return; }
      try {
        const result = await identityCenterApi.poll(login.session_id);
        if (disposed || request !== generation.current) return;
        if (result.status === 'authorized') {
          const available = await identityCenterApi.accounts(login.session_id);
          if (!disposed && request === generation.current) setAccounts(available);
        } else {
          timer = setTimeout(poll, Math.max(1, result.interval) * 1000);
        }
      } catch (error) { fail(error); }
    };
    timer = setTimeout(poll, Math.max(1, login.interval) * 1000);
    return () => { disposed = true; clearTimeout(timer); };
  }, [login, accounts]);

  const start = async () => {
    cancel(); setError(null); setNotice(null); setBusy(true);
    const request = generation.current;
    try {
      const result = await identityCenterApi.start(startUrl.trim(), ssoRegion.trim());
      if (request !== generation.current) { await identityCenterApi.cancel(result.session_id); return; }
      session.current = result.session_id;
      setLogin(result);
    } catch (error) { if (request === generation.current) setError(message(error)); }
    finally { if (request === generation.current) setBusy(false); }
  };

  const selectAccount = async (id: string) => {
    const request = ++roleGeneration.current;
    setAccountId(id); setRole(''); setRoles([]); setLoadingRoles(true); setError(null);
    try {
      const available = await identityCenterApi.roles(login!.session_id, id);
      if (request === roleGeneration.current) setRoles(available);
    } catch (error) { if (request === roleGeneration.current) setError(message(error)); }
    finally { if (request === roleGeneration.current) setLoadingRoles(false); }
  };

  const save = async () => {
    if (!login || !name.trim() || !accountId || !role) return;
    const request = generation.current;
    setBusy(true); setError(null);
    try {
      const saved = await identityCenterApi.save(login.session_id, accountId, role, name.trim(), region, profile?.id);
      if (request !== generation.current) return;
      // Native persistence now owns this session; closing the form must not cancel it.
      session.current = null;
      await onSaved(saved);
    } catch (error) { if (request === generation.current) setError(message(error)); }
    finally { if (request === generation.current) setBusy(false); }
  };

  return <Box sx={{ display: 'grid', gap: 2 }}>
    <Alert severity="info">Sign in with your work account in a browser. No AWS CLI, access keys or administrator installation is needed. Your AWS administrator must already have assigned an account and role.</Alert>
    {existing && <Typography variant="body2">Saved AWS connection: {existing.account_id} · {existing.role_name}. Sign in again here when your session expires.</Typography>}
    <TextField label="AWS access portal URL" placeholder="https://your-company.awsapps.com/start" value={startUrl} disabled={!!login || busy} onChange={e => setStartUrl(e.target.value)} />
    <TextField label="IAM Identity Center region" helperText="The region of your company's AWS sign-in portal; it may differ from your bucket region." value={ssoRegion} disabled={!!login || busy} onChange={e => setSsoRegion(e.target.value)} />
    {!login && <Button variant="contained" disabled={busy || !startUrl.trim() || !ssoRegion.trim()} onClick={start}>{busy ? 'Preparing sign-in…' : 'Start browser sign-in'}</Button>}
    {login && accounts === null && <Box sx={{ display: 'grid', gap: 2, border: '1px solid', borderColor: 'divider', p: 2, borderRadius: 2 }}>
      <Typography variant="subtitle2">Approve this code on the AWS page</Typography>
      <Typography component="code" sx={{ fontSize: 24, letterSpacing: 3 }}>{login.user_code}</Typography>
      <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{login.verification_uri}</Typography>
      <Button variant="outlined" onClick={async () => { const request = generation.current; try { await identityCenterApi.openBrowser(login.session_id); } catch (error) { if (request === generation.current) setError(message(error)); } }}>Open AWS sign-in in browser</Button>
      <Typography variant="body2"><CircularProgress size={14} sx={{ mr: 1 }} />Waiting for approval. This code expires in {Math.ceil(login.expires_in / 60)} minutes.</Typography>
    </Box>}
    {accounts !== null && <>
      {accounts.length === 0 ? <Alert severity="warning">No AWS accounts are assigned to this user. Ask your AWS administrator to assign access, then sign in again.</Alert> : <TextField select label="AWS account" value={accountId} disabled={busy} onChange={e => void selectAccount(e.target.value)}>{accounts.map(account => <MenuItem key={account.account_id} value={account.account_id}>{account.account_name || account.account_id} ({account.account_id})</MenuItem>)}</TextField>}
      {loadingRoles && <Typography>Loading assigned roles…</Typography>}
      {accountId && !loadingRoles && (roles.length ? <TextField select label="AWS role" value={role} disabled={busy} onChange={e => setRole(e.target.value)}>{roles.map(item => <MenuItem key={item.role_name} value={item.role_name}>{item.role_name}</MenuItem>)}</TextField> : <Alert severity="warning">No roles are assigned for this account. Choose another account or contact your AWS administrator.</Alert>)}
      <Button variant="contained" disabled={busy || !role || !name.trim() || loadingRoles} onClick={save}>{busy ? 'Saving connection…' : 'Save AWS connection'}</Button>
    </>}
    {login && <Button onClick={cancel} disabled={busy}>Cancel sign-in</Button>}
    {existing && !login && <Button disabled={busy} onClick={async () => { const request = generation.current; setBusy(true); setError(null); try { await identityCenterApi.signOut(profile!.id); if (request === generation.current) setNotice('Signed out on this computer. Sign in again to use this profile.'); } catch (error) { if (request === generation.current) setError(message(error)); } finally { if (request === generation.current) setBusy(false); } }}>Sign out on this computer</Button>}
    {notice && <Alert severity="success">{notice}</Alert>}
    {error && <Alert severity="error">{error}</Alert>}
  </Box>;
}
