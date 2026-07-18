const { defineConfig } = require('@yarnpkg/types');

/**
 * Check peer dependencies is correct.
 * @param {Constraints.Context} context
 * @returns {Promise<void>}
 */
async function checkPeerDependencies(context) {
  const { Yarn } = context;

  for (const dependency of Yarn.dependencies({ type: 'peerDependencies' })) {
    const { workspace, ident } = dependency;

    const production = Yarn.dependency({ workspace, ident, type: 'dependencies' });
    if (production !== null) {
      dependency.error(
        `${ident} must not be listed as both a dependency and a peer dependency`
      );
      continue;
    }

    const optional = workspace.manifest.peerDependenciesMeta?.[ident]?.optional === true;
    if (optional) {
      continue;
    }

    const development = Yarn.dependency({ workspace, ident, type: 'devDependencies' });
    if (development === null) {
      dependency.error(
        `${ident} is a peer dependency and must also be listed in devDependencies`
      );
    }
  }

  const rangesByIdent = new Map();
  for (const dependency of Yarn.dependencies({ type: 'peerDependencies' })) {
    const ranges = rangesByIdent.get(dependency.ident) ?? new Map();
    ranges.set(dependency.range, [...(ranges.get(dependency.range) ?? []), dependency]);
    rangesByIdent.set(dependency.ident, ranges);
  }
  for (const [ident, ranges] of rangesByIdent) {
    if (ranges.size <= 1) {
      continue;
    }
    const found = Array.from(ranges.keys()).sort().join(', ');
    for (const owners of ranges.values()) {
      for (const dependency of owners) {
        dependency.error(`${ident} has inconsistent peer dependency ranges (${found})`);
      }
    }
  }
}

/**
 * Check playground packages depend on workspaces the way a published consumer would.
 *
 * The playground exists to exercise the packages through their public surface, so it pins a
 * version range instead of the `workspace:` protocol — a range a real consumer could write.
 * @param {Constraints.Context} context
 * @returns {Promise<void>}
 */
async function checkPlaygroundDependencies(context) {
  const { Yarn } = context;

  for (const dependency of Yarn.dependencies()) {
    if (!dependency.workspace.cwd.startsWith('playground/')) {
      continue;
    }
    if (!dependency.range.startsWith('workspace:')) {
      continue;
    }
    dependency.error(
      `${dependency.ident} must not use the workspace protocol (${dependency.range}) in playground; pin a version range instead`
    );
  }
}

module.exports = defineConfig({
  async constraints(context) {
    await Promise.allSettled([
      checkPeerDependencies(context),
      checkPlaygroundDependencies(context)
    ]);
  }
});
