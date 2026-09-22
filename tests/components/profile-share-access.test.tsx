import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import ProfileDialog from '@/components/profile/ProfileDialog';
import { profileApi } from '@/lib/tauri';
import { useProfileStore } from '@/store/profileStore';

vi.mock('next/navigation', () => ({ useRouter: () => ({ push: vi.fn() }) }));

vi.mock('@/lib/tauri', async importOriginal => {
  const actual = await importOriginal<typeof import('@/lib/tauri')>();
  return { ...actual, profileApi: {
    ...actual.profileApi,
    discoverLocalProfiles: vi.fn().mockResolvedValue([]),
    checkAwsEnvironment: vi.fn().mockResolvedValue({ has_access_key: false, has_secret_key: false, has_session_token: false }),
    testConnection: vi.fn().mockResolvedValue({ success: false, message: 'ListBuckets denied' }),
    addProfile: vi.fn().mockResolvedValue({ id: 'scoped', name: 'Folder only', credential_type: { type: 'Environment' }, is_default: true, region: 'us-east-1' }),
    setActiveProfile: vi.fn().mockResolvedValue(undefined),
  } };
});
beforeEach(() => useProfileStore.setState({ profiles: [], activeProfileId: null }));

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
