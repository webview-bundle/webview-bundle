#!/usr/bin/env node
// `create-webview-bundle` exists only so `npm create webview-bundle` / `yarn create webview-bundle`
// resolve. All logic and templates live in create-wvb; importing its CLI runs it against this
// process's argv, and its bundled templates resolve next to its own dist.
import 'create-wvb/cli';
