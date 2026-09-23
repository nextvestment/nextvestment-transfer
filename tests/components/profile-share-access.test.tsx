import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import ProfileDialog from '@/components/profile/ProfileDialog';
import { bucketApi, profileApi } from '@/lib/tauri';
import { useProfileStore } from '@/store/profileStore';
import { useProjectShareStore } from '@/store/projectShareStore';

vi.mock('next/navigation', () => ({ useRouter: () => ({ push: vi.fn() }) }));

vi.mock('@/lib/tauri', async importOriginal => {
  const actual = await importOriginal<typeof import('@/lib/tauri')>();
  return { ...actual, bucketApi: { ...actual.bucketApi, refreshS3Client: vi.fn().mockResolvedValue(undefined) }, profileApi: {
    ...actual.profileApi,
    discoverLocalProfiles: vi.fn().mockResolvedValue([]),
    checkAwsEnvironment: vi.fn().mockResolvedValue({ has_access_key: false, has_secret_key: false, has_session_token: false }),
    testConnection: vi.fn().mockResolvedValue({ success: false, message: 'ListBuckets denied' }),
    addProfile: vi.fn().mockResolvedValue({ id: 'scoped', name: 'Folder only', credential_type: { type: 'Environment' }, is_default: true, region: 'us-east-1' }),
    setActiveProfile: vi.fn().mockResolvedValue(undefined),
  } };
});
beforeEach(() => {
  useProfileStore.setState({ profiles: [], activeProfileId: null });
  useProjectShareStore.setState({ shares: {} });
  localStorage.clear();
});

test('a profile can be saved even when optional account-wide discovery is denied', async () => {
  render(<ProfileDialog open onClose={vi.fn()} />);
  fireEvent.click(await screen.findByRole('button', { name: 'Create New Profile' }));
  fireEvent.mouseDown(screen.getAllByRole('combobox')[0]);
  fireEvent.click(await screen.findByRole('option', { name: 'System Environment Variables' }));
  fireEvent.change(screen.getByLabelText(/Profile Name/), { target: { value: 'Folder only' } });
  fireEvent.click(screen.getByRole('button', { name: 'Test bucket discovery (optional)' }));
  await screen.findByText('Discovery unavailable');
  fireEvent.click(screen.getByRole('button', { name: 'Connect Account' }));
  await waitFor(() => expect(profileApi.addProfile).toHaveBeenCalledOnce());
  expect(useProfileStore.getState().activeProfileId).toBe('scoped');
  expect(profileApi.testConnection).toHaveBeenCalledOnce();
});

test('one paste creates the vault-backed profile and project share without storing the secret in browser storage', async () => {
  const secret = 'a'.repeat(40);
  vi.mocked(profileApi.addProfile).mockResolvedValueOnce({
    id: 'scoped', name: 'Folder only', credential_type: { type: 'Manual', access_key_id: `AKIA${'A'.repeat(16)}`, secret_access_key: secret },
    is_default: true, region: 'ap-southeast-1',
  });
  render(<ProfileDialog open onClose={vi.fn()} />);
  fireEvent.click(await screen.findByRole('button', { name: 'Paste shared setup' }));
  fireEvent.change(screen.getByLabelText('Shared setup JSON'), { target: { value: JSON.stringify({
    type: 'nextvestment-transfer-share', version: 1, name: 'Folder only',
    bucket: 'project-files', region: 'ap-southeast-1', prefix: 'team/',
    accessKeyId: `AKIA${'A'.repeat(16)}`, secretAccessKey: secret,
  }) } });
  fireEvent.click(screen.getByRole('button', { name: 'Save shared setup' }));
  await waitFor(() => expect(profileApi.addProfile).toHaveBeenCalledWith(expect.objectContaining({
    credential_type: { type: 'Manual', access_key_id: `AKIA${'A'.repeat(16)}`, secret_access_key: secret },
  })));
  await waitFor(() => expect(useProjectShareStore.getState().shares.scoped).toEqual({
    bucket: 'project-files', region: 'ap-southeast-1', prefix: 'team/', autoRefresh: true,
  }));
  expect(useProfileStore.getState().activeProfileId).toBe('scoped');
  expect(useProfileStore.getState().profiles[0].credential_type).toEqual({
    type: 'Manual', access_key_id: `AKIA${'A'.repeat(16)}`, secret_access_key: '',
  });
  expect(bucketApi.refreshS3Client).toHaveBeenCalled();
  const storedValues = Array.from({ length: localStorage.length }, (_, index) => localStorage.getItem(localStorage.key(index)!));
  expect(storedValues.join(' ')).not.toContain(secret);
});
