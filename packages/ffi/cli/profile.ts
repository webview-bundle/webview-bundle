import path from 'node:path';
import { Option } from 'clipanion';
import { isEnum } from 'typanion';
import { z } from 'zod';
import { ROOT_DIR } from './consts.ts';

export const ProfileSchema = z.enum(['dev', 'release']);
export type Profile = z.infer<typeof ProfileSchema>;

export const ProfileOption = Option.String('--profile', 'dev', {
  validator: isEnum(ProfileSchema.options),
  description: 'Set the profile to use. [Default: "dev"]',
});

export function getProfileTargetDir(profile: Profile, target?: string): string {
  const targetDir = path.join(ROOT_DIR, 'target');
  const profileDir = profile === 'dev' ? 'debug' : 'release';

  if (target != null) {
    return path.join(targetDir, target, profileDir);
  }

  return path.join(targetDir, profileDir);
}
