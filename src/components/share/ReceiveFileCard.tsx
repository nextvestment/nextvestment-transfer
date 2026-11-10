"use client";

import { useState } from 'react';
import { Alert, Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { receiveHandoff } from '@/lib/tauri';

export default function ReceiveFileCard() {
  const [link, setLink] = useState('');
  const [checksum, setChecksum] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [receipt, setReceipt] = useState<{ bytes: number; sha256: string } | null>(null);
  async function receive() {
    setError(''); setReceipt(null); setBusy(true);
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const path = await save({ title: 'Save received file', defaultPath: 'received-file.zip' });
      if (!path) return;
      const result = await receiveHandoff(link.trim(), path, checksum.trim() || undefined);
      setReceipt(result); setLink(''); setChecksum('');
    } catch {
      // Native errors can contain local paths; never echo arbitrary error objects or URLs.
      setError('Could not receive the file. Check the link and checksum, choose a new filename, or ask the sender for a fresh link.');
    } finally { setBusy(false); }
  }
  return <Paper variant="outlined" component="section" aria-labelledby="receive-heading" sx={{ p: 3, mb: 3, width: '100%' }}>
    <Stack spacing={2}>
      <Typography id="receive-heading" variant="h5">Receive a file</Typography>
      <Typography variant="body2">Paste the download link from the sender’s Mac. No AWS account or sign-in is needed on this computer.</Typography>
      <TextField label="Private download link" type="password" value={link} onChange={event => setLink(event.target.value)} disabled={busy} autoComplete="off" helperText="The link grants access to one file until it expires. Keep it private." />
      <TextField label="SHA-256 checksum (optional)" value={checksum} onChange={event => setChecksum(event.target.value)} disabled={busy} autoComplete="off" />
      <Button variant="contained" onClick={receive} disabled={busy || !link.trim()}>{busy ? 'Receiving file…' : 'Choose destination and receive'}</Button>
      {error && <Alert severity="error">{error}</Alert>}
      {receipt && <Alert severity="success" sx={{ overflowWrap: 'anywhere' }}>File received: {receipt.bytes.toLocaleString()} bytes. SHA-256: {receipt.sha256}</Alert>}
    </Stack>
  </Paper>;
}
