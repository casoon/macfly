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
        'Let AI work with your Mac. On your terms. A controlled, local AI access layer for Apple Notes, Mail, Contacts, Calendar, Reminders, Safari, and Shortcuts.',
      tagline: 'Let AI work with your Mac. On your terms.',
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
