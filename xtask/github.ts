import { retry } from '@octokit/plugin-retry';
import { Octokit } from '@octokit/rest';

export type GitHubClient = Octokit;

export function createGitHubClient(token: string): GitHubClient {
  const Client = Octokit.plugin(retry);
  return new Client({ auth: token, userAgent: 'webview-bundle' });
}
