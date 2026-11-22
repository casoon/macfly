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
        'Stay in control when AI uses your Mac — policy-guarded macOS automation, local data redaction, reviewable change plans, and Markdown migrations for Apple Notes, Mail, Contacts, Calendar, and Reminders.',
      tagline: 'Stay in control when AI uses your Mac',
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
