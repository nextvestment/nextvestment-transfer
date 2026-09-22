import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import IdentityCenterSetup from '@/components/profile/IdentityCenterSetup';
import { identityCenterApi } from '@/lib/tauri';

vi.mock('@/lib/tauri', async importOriginal => ({
  ...await importOriginal<typeof import('@/lib/tauri')>(),
  identityCenterApi: { start: vi.fn(), poll: vi.fn(), openBrowser: vi.fn(), accounts: vi.fn(), roles: vi.fn(), save: vi.fn(), cancel: vi.fn().mockResolvedValue(undefined), signOut: vi.fn().mockResolvedValue(undefined) },
}));
const login = { session_id: 'opaque-session', user_code: 'ABCD-EFGH', verification_uri: 'https://device.sso.ap-southeast-1.amazonaws.com/', expires_in: 600, interval: 1 };
const profile = { id: 'saved', name: 'Team', region: 'ap-southeast-1', is_default: false, credential_type: { type: 'IdentityCenter' as const, start_url: 'https://company.awsapps.com/start', sso_region: 'ap-southeast-1', account_id: '123456789012', role_name: 'Transfer', session_ref: 'opaque-session' } };
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(identityCenterApi.start).mockResolvedValue(login);
  vi.mocked(identityCenterApi.poll).mockResolvedValue({ status: 'authorized', interval: 1 });
  vi.mocked(identityCenterApi.accounts).mockResolvedValue([{ account_id: '123456789012', account_name: 'Team' }]);
  vi.mocked(identityCenterApi.roles).mockResolvedValue([{ account_id: '123456789012', role_name: 'Transfer' }]);
  vi.mocked(identityCenterApi.save).mockResolvedValue(profile);
});
afterEach(() => vi.useRealTimers());
async function start() {
  fireEvent.change(screen.getByLabelText('AWS access portal URL'), { target: { value: profile.credential_type.start_url } });
  fireEvent.click(screen.getByRole('button', { name: 'Start browser sign-in' }));
  await screen.findByText('ABCD-EFGH');
}
test('browser approval leads to explicit assigned account and role selection, with no keys requested', async () => {
  const onSaved = vi.fn().mockResolvedValue(undefined);
  render(<IdentityCenterSetup name="Team" region="ap-southeast-1" onSaved={onSaved} />);
  await start();
  fireEvent.click(screen.getByRole('button', { name: 'Open AWS sign-in in browser' }));
  expect(identityCenterApi.openBrowser).toHaveBeenCalledWith('opaque-session');
  const account = await screen.findByRole('combobox', { name: 'AWS account' }, { timeout: 3000 });
  fireEvent.mouseDown(account); fireEvent.click(await screen.findByRole('option', { name: 'Team (123456789012)' }));
  fireEvent.mouseDown(await screen.findByRole('combobox', { name: 'AWS role' })); fireEvent.click(await screen.findByRole('option', { name: 'Transfer' }));
  fireEvent.click(screen.getByRole('button', { name: 'Save AWS connection' }));
  await waitFor(() => expect(onSaved).toHaveBeenCalledWith(profile));
  expect(identityCenterApi.save).toHaveBeenCalledWith('opaque-session', '123456789012', 'Transfer', 'Team', 'ap-southeast-1', undefined);
  expect(screen.queryByLabelText('Secret Access Key')).toBeNull();
});
test('closing during device authorization cancels and ignores late results', async () => {
  let resolve!: (value: typeof login) => void;
  vi.mocked(identityCenterApi.start).mockReturnValue(new Promise(r => { resolve = r; }));
  const { unmount } = render(<IdentityCenterSetup name="Team" region="ap-southeast-1" onSaved={vi.fn()} />);
  fireEvent.change(screen.getByLabelText('AWS access portal URL'), { target: { value: profile.credential_type.start_url } });
  fireEvent.click(screen.getByRole('button', { name: 'Start browser sign-in' }));
  unmount();
  await act(async () => resolve(login));
  expect(identityCenterApi.cancel).toHaveBeenCalledWith('opaque-session');
  expect(identityCenterApi.poll).not.toHaveBeenCalled();
});
test('expired sign-in stops polling and permits a fresh authorization', async () => {
  vi.mocked(identityCenterApi.start).mockResolvedValue({ ...login, expires_in: 0 });
  render(<IdentityCenterSetup name="Team" region="ap-southeast-1" onSaved={vi.fn()} />);
  await start();
  await screen.findByText('The browser sign-in expired. Start again to receive a new code.', {}, { timeout: 3000 });
  expect(identityCenterApi.poll).not.toHaveBeenCalled();
  expect(identityCenterApi.cancel).toHaveBeenCalledWith('opaque-session');
  expect(screen.getByRole('button', { name: 'Start browser sign-in' })).toBeTruthy();
});
test('saved profile metadata is readable after restart and sign-out targets that profile', async () => {
  render(<IdentityCenterSetup name="Team" region="ap-southeast-1" profile={profile} onSaved={vi.fn()} />);
  expect(screen.getByLabelText('AWS access portal URL')).toHaveProperty('value', profile.credential_type.start_url);
  expect(screen.getByText(/123456789012 · Transfer/)).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: 'Sign out on this computer' }));
  await screen.findByText('Signed out on this computer. Sign in again to use this profile.');
  expect(identityCenterApi.signOut).toHaveBeenCalledWith('saved');
});
