import assert from 'node:assert/strict';
import test from 'node:test';
import { sameRustBuild } from '../src/lib/rustVersion.ts';

test('identical full Rust builds match despite the rustc prefix', () => {
  assert.equal(sameRustBuild('rustc 1.99.1 (abcdef1234 2026-11-12)', '1.99.1 (abcdef1234 2026-11-12)'), true);
});
test('different nightly commits are updates even with the same release number', () => {
  assert.equal(sameRustBuild('rustc 1.101.0-nightly (1111111111 2026-11-12)', '1.101.0-nightly (2222222222 2026-11-13)'), false);
});
test('an old stable release is not up to date', () => {
  assert.equal(sameRustBuild('rustc 1.99.0 (1111111111 2026-11-01)', '1.99.1 (2222222222 2026-11-12)'), false);
});
test('missing nightly build identity is not proof of being up to date', () => {
  assert.equal(sameRustBuild('1.101.0-nightly', '1.101.0-nightly'), false);
  assert.equal(sameRustBuild('1.101.0-nightly', '1.101.0-nightly (2222222222 2026-11-13)'), false);
});
test('stable release-only fallbacks and shortened commit hashes are supported', () => {
  assert.equal(sameRustBuild('rustc 1.99.1', '1.99.1'), true);
  assert.equal(sameRustBuild('rustc 1.99.1 (abcdef1234 2026-11-12)', '1.99.1 (abcdef1 2026-11-12)'), true);
});
test('missing versions never match', () => {
  assert.equal(sameRustBuild(null, '1.99.1'), false);
  assert.equal(sameRustBuild('1.99.1', undefined), false);
  assert.equal(sameRustBuild('', ''), false);
});
