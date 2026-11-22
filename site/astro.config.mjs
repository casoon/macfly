// @ts-check
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://casoon.github.io/macfly',
  base: '/macfly/',
  integrations: [
    casoonPages({
      name: 'MacFly',
      description:
        'Safe, deterministic macOS automation CLI, two-way migration toolkit & MCP server in pure Rust — with a read-side DLP firewall and SHA-256 plan approval.',
      tagline: 'Safe, deterministic macOS automation & migration',
      repo: 'casoon/macfly',
      license: 'MIT OR Apache-2.0',
      packages: [
        { label: 'crates.io', href: 'https://crates.io/crates/macfly' },
        { label: 'docs.rs', href: 'https://docs.rs/macfly' },
        { label: 'GitHub Releases', href: 'https://github.com/casoon/macfly/releases' },
      ],
      demo: 'Use Cases',
      showcase: false,
      changelog: '../CHANGELOG.md',
      docsGroups: {
        guide: 'Guide',
        architecture: 'Architecture',
      },
    }),
  ],
});
