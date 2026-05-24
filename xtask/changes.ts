/**
 * A single changelog entry. Either a real commit (with its sha) or a synthetic note such as a
 * dependency bump generated when a package is released only because one of its dependencies
 * changed. The raw commit summary is used verbatim — there is no conventional-commit parsing.
 */
export class Change {
  readonly summary: string;
  readonly sha?: string;

  constructor(summary: string, sha?: string) {
    this.summary = summary;
    this.sha = sha;
  }

  toString(): string {
    return this.sha != null ? `${this.summary} (${this.sha.slice(0, 7)})` : this.summary;
  }
}

export class Changes {
  readonly changes: readonly Change[];

  constructor(changes: Change[]) {
    this.changes = changes;
  }

  get isEmpty(): boolean {
    return this.changes.length === 0;
  }
}
