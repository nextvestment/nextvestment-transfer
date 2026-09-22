'use client';

import { useState } from 'react';
import { Alert, Box, Button, Checkbox, FormControlLabel, Paper, Stack, TextField, Typography } from '@mui/material';
import { FolderShared, ArrowForward, EditOutlined } from '@mui/icons-material';
import { useRouter } from 'next/navigation';
import { useAppStore } from '@/store/appStore';
import { projectSharePath, useProjectShareStore } from '@/store/projectShareStore';

export default function ProjectShareCard({ profileId, profileName, defaultRegion }: { profileId: string; profileName: string; defaultRegion: string }) {
  const saved = useProjectShareStore(state => state.shares[profileId]);
  const saveShare = useProjectShareStore(state => state.saveShare);
  const [editing, setEditing] = useState(!saved);
  const [bucket, setBucket] = useState(saved?.bucket ?? '');
  const [region, setRegion] = useState(saved?.region ?? defaultRegion);
  const [prefix, setPrefix] = useState(saved?.prefix ?? '');
  const [autoRefresh, setAutoRefresh] = useState(saved?.autoRefresh ?? true);
  const [error, setError] = useState('');
  const router = useRouter();
  const openShare = () => {
    const share = useProjectShareStore.getState().shares[profileId];
    if (!share) return;
    const path = projectSharePath(share);
    useAppStore.getState().addTab({ title: 'Project Share', path, icon: 'bucket' });
    router.push(path);
  };

  return (
    <Paper component="section" aria-labelledby="project-share-heading" variant="outlined" sx={{ p: { xs: 2.5, sm: 4 }, mb: 3, borderTop: 4, borderTopColor: 'primary.main', borderRadius: 2 }}>
      <Stack direction="row" spacing={1.5} alignItems="center" sx={{ mb: 1.5 }}>
        <FolderShared color="primary" fontSize="large" />
        <Box>
          <Typography variant="overline" color="text.secondary">NEXTVESTMENT TRANSFER</Typography>
          <Typography id="project-share-heading" variant="h5" fontWeight={750}>Project Share</Typography>
        </Box>
      </Stack>
      <Typography color="text.secondary" variant="body2" sx={{ mb: 3 }}>
        A shared project folder for this computer and your VDI. Upload here, then open the same folder on the other computer to download it.
      </Typography>
      <Typography variant="body2" sx={{ mb: 2 }}>AWS profile: <strong>{profileName}</strong></Typography>
      {editing ? (
        <Box component="form" onSubmit={event => {
          event.preventDefault();
          try {
            saveShare(profileId, { bucket, region, prefix, autoRefresh });
            setError('');
            setEditing(false);
          } catch (cause) {
            setError(cause instanceof Error ? cause.message : 'Could not save the shared folder.');
          }
        }}>
          <Stack spacing={2}>
            <TextField label="Bucket name" value={bucket} onChange={event => setBucket(event.target.value)} required fullWidth helperText="Use the existing project bucket supplied by your administrator." />
            <TextField label="AWS region" value={region} onChange={event => setRegion(event.target.value)} required fullWidth />
            <TextField label="Folder prefix" value={prefix} onChange={event => setPrefix(event.target.value)} fullWidth helperText="For example project-share/. Leave empty only when access is granted to the bucket root." />
            <FormControlLabel control={<Checkbox checked={autoRefresh} onChange={event => setAutoRefresh(event.target.checked)} />} label="Check for shared files every 15 seconds while this folder is open" />
            {error && <Alert severity="error">{error}</Alert>}
            <Stack direction="row" spacing={1}>
              <Button type="submit" variant="contained" disableElevation>Save shared folder</Button>
              {saved && <Button onClick={() => { setEditing(false); setError(''); }}>Cancel</Button>}
            </Stack>
          </Stack>
        </Box>
      ) : saved && (
        <>
          <Typography component="p" sx={{ p: 2, mb: 2, bgcolor: 'action.hover', borderRadius: 1, fontFamily: 'monospace', overflowWrap: 'anywhere' }}>s3://{saved.bucket}/{saved.prefix}</Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>{saved.region} · {saved.autoRefresh ? 'Automatic refresh enabled' : 'Refresh manually'}</Typography>
          <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
            <Button variant="contained" endIcon={<ArrowForward />} onClick={openShare} disableElevation>Open Project Share</Button>
            <Button startIcon={<EditOutlined />} onClick={() => { setBucket(saved.bucket); setRegion(saved.region); setPrefix(saved.prefix); setAutoRefresh(saved.autoRefresh); setEditing(true); }}>Edit setup</Button>
            <Button color="inherit" onClick={() => { useProjectShareStore.getState().removeShare(profileId); setEditing(true); }}>Forget setup</Button>
          </Stack>
        </>
      )}
      <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 2.5 }}>
        Only the folder setup is saved here; credentials stay in your AWS profile. Opening this folder does not list all buckets. Files transfer when you upload or download; this is not automatic disk sync.
      </Typography>
    </Paper>
  );
}
