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
        'Safe, fast, AI-ready macOS automation CLI & data firewall in pure Rust — for Apple Notes, Apple Mail, and local agents.',
      repo: 'casoon/macfly',
      license: 'MIT OR Apache-2.0',
      changelog: false,
      showcase: false,
      docsGroups: {
        guide: 'Guide',
        architecture: 'Architecture',
      },
    }),
  ],
});
