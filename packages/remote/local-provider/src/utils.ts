import type { DeploymentFile } from '@wvb/remote-local/api';

export function getRemoteBundleDeploymentVersion(
  deployment: DeploymentFile,
  channel?: string,
  fallback = false
): string | undefined {
  if (channel != null) {
    const channelVersion = deployment.channels?.[channel];
    if (channelVersion != null) {
      return channelVersion;
    }
    if (!fallback) {
      return undefined;
    }
  }
  return deployment.version;
}
