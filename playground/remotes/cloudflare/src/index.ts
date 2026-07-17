import { wvbRemote } from '@wvb/remote-cloudflare-provider';

const remote = wvbRemote();

export default {
  fetch: remote.fetch,
};
