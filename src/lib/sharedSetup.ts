import { ProjectShare, validateProjectShare } from '@/store/projectShareStore';

export interface SharedSetup {
  name: string;
  accessKeyId: string;
  secretAccessKey: string;
  share: ProjectShare;
}

export class SharedSetupError extends Error {}

const FIELDS = new Set(['type', 'version', 'name', 'bucket', 'region', 'prefix', 'accessKeyId', 'secretAccessKey']);

export function parseSharedSetup(input: string): SharedSetup {
  if (input.length > 4096) throw new SharedSetupError('The setup string is too long.');
  let value: unknown;
  try {
    value = JSON.parse(input);
  } catch {
    throw new SharedSetupError('Paste the complete Transfer setup JSON.');
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new SharedSetupError('Transfer setup must be a JSON object.');
  }
  const data = value as Record<string, unknown>;
  if (Object.keys(data).some(key => !FIELDS.has(key)) || data.type !== 'nextvestment-transfer-share' || data.version !== 1) {
    throw new SharedSetupError('This is not a supported Transfer shared-folder setup.');
  }
  const name = data.name;
  const accessKeyId = data.accessKeyId;
  const secretAccessKey = data.secretAccessKey;
  if (typeof name !== 'string' || !name.trim() || name.length > 80 || /[\x00-\x1f\x7f]/.test(name)) {
    throw new SharedSetupError('The setup needs a valid profile name.');
  }
  // AWS IAM user keys use AKIA. ASIA keys are temporary STS sessions and also need a session token.
  if (typeof accessKeyId !== 'string' || !/^AKIA[A-Z0-9]{16}$/.test(accessKeyId)) {
    throw new SharedSetupError('Use a long-term IAM access key (AKIA), not an expiring session key.');
  }
  if (typeof secretAccessKey !== 'string' || !/^[A-Za-z0-9/+=]{40}$/.test(secretAccessKey)) {
    throw new SharedSetupError('The secret access key must be a complete 40-character AWS key.');
  }
  if (typeof data.bucket !== 'string' || typeof data.region !== 'string' || typeof data.prefix !== 'string') {
    throw new SharedSetupError('The setup needs a bucket, region and folder prefix.');
  }
  let share: ProjectShare;
  try {
    share = validateProjectShare({ bucket: data.bucket, region: data.region, prefix: data.prefix, autoRefresh: true });
  } catch (cause) {
    throw new SharedSetupError(cause instanceof Error ? cause.message : 'The shared folder is invalid.');
  }
  if (!share.prefix) throw new SharedSetupError('The shared folder prefix cannot be empty.');
  return { name: name.trim(), accessKeyId, secretAccessKey, share };
}
