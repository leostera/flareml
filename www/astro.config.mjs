import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import fmlLanguage from './src/langs/fml.mjs';
import fmlTheme from './src/langs/fml-theme.mjs';

export default defineConfig({
  site: 'https://flareml.leostera.dev',
  integrations: [
    starlight({
      title: 'FlareML',
      description: 'A finite systems modeling language with a native Rust model checker.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
      },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/leostera/flareml' },
      ],
      customCss: ['./src/styles/custom.css'],
      components: {
        Footer: './src/components/SiteFooter.astro',
        SiteTitle: './src/components/SiteTitle.astro',
      },
      expressiveCode: {
        shiki: { langs: [fmlLanguage] },
        themes: [fmlTheme],
        useStarlightDarkModeSwitch: false,
      },
      sidebar: [
        {
          label: 'Get started',
          items: [
            { slug: 'guide/getting-started' },
            { slug: 'guide/link-shortener' },
          ],
        },
        {
          label: 'From other tools',
          items: [
            { slug: 'guide/from-other-languages', label: 'Overview' },
            { slug: 'guide/from-tla-plus' },
            { slug: 'guide/from-alloy' },
            { slug: 'guide/from-quint' },
            { slug: 'guide/from-z3' },
            { slug: 'guide/from-lean' },
          ],
        },
        {
          label: 'Language & execution',
          items: [
            { slug: 'guide/language', label: 'Language overview' },
            { slug: 'guide/execution-model', label: 'How models run' },
            { slug: 'reference/syntax', label: 'Syntax' },
            { slug: 'reference/actors', label: 'Actors and turns' },
            { slug: 'reference/properties', label: 'Properties and time' },
            { slug: 'reference/checks', label: 'Checks, bounds, and setup' },
            { slug: 'reference/observations', label: 'Inputs and messages' },
            { slug: 'reference/cli', label: 'Traces and replay' },
          ],
        },
        {
          label: 'Examples',
          items: [
            { slug: 'examples', label: 'Overview' },
            { label: 'Atomic increments', link: 'https://github.com/leostera/flareml/blob/main/examples/atomic-increments.fml' },
            { label: 'Counter replies', link: 'https://github.com/leostera/flareml/blob/main/examples/counter-replies.fml' },
            { label: 'Eligibility check', link: 'https://github.com/leostera/flareml/blob/main/examples/eligibility-check.fml' },
            { label: 'Explicit startup', link: 'https://github.com/leostera/flareml/blob/main/examples/explicit-startup.fml' },
            { label: 'Faulty link: loss', link: 'https://github.com/leostera/flareml/blob/main/examples/faulty-link-loss.fml' },
            { label: 'Faulty link: duplicate bug', link: 'https://github.com/leostera/flareml/blob/main/examples/faulty-link-duplicate-bug.fml' },
            { label: 'Faulty link: duplicate repair', link: 'https://github.com/leostera/flareml/blob/main/examples/faulty-link-duplicate-fixed.fml' },
            { label: 'Inventory reservation bug', link: 'https://github.com/leostera/flareml/blob/main/examples/inventory-reservation-bug.fml' },
            { label: 'Inventory reservation repair', link: 'https://github.com/leostera/flareml/blob/main/examples/inventory-reservation-fixed.fml' },
            { label: 'Link shortener', link: '/guide/link-shortener/' },
            { label: 'Lost update', link: 'https://github.com/leostera/flareml/blob/main/examples/lost-update.fml' },
            { label: 'Missing reply', link: 'https://github.com/leostera/flareml/blob/main/examples/missing-reply.fml' },
            { label: 'Payment idempotency bug', link: 'https://github.com/leostera/flareml/blob/main/examples/payment-idempotency-bug.fml' },
            { label: 'Payment idempotency repair', link: 'https://github.com/leostera/flareml/blob/main/examples/payment-idempotency-fixed.fml' },
            { label: 'Routed deposits', link: 'https://github.com/leostera/flareml/blob/main/examples/routed-deposits.fml' },
            { label: 'Sequential workflow', link: 'https://github.com/leostera/flareml/blob/main/examples/sequential-workflow.fml' },
            { label: 'Spawn choice workers', link: 'https://github.com/leostera/flareml/blob/main/examples/spawn-choice-workers.fml' },
            { label: 'Spawned workers', link: 'https://github.com/leostera/flareml/blob/main/examples/spawn-workers.fml' },
          ],
        },
      ],
    }),
  ],
});
