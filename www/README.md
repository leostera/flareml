# FlareML website

A static Astro + Starlight site, built with Bun and deployed as a Cloudflare Worker with Workers Static Assets.

## Develop

```sh
bun install
bun run dev
```

## Check and build

```sh
bun run check
bun run build
```

Astro outputs the site to `dist/`. Wrangler serves and deploys that directory as static assets; there is no SSR Worker or runtime adapter because the site is fully static. The FML code fence language and its restrained monochrome Shiki theme live in `src/langs/`. Reference pages under `src/content/docs/reference/` are symlinks to `../docs/skills/fml/`; the website and `fml skills TOPIC` therefore share the same manual source.

## Deploy

```sh
bun run deploy
```

`wrangler.jsonc` names the Worker `flareml-www` and maps the custom domain `flareml.leostera.dev`. Before the first deploy, the Cloudflare account must be authenticated with Wrangler and the domain must be an available zone in that account. The custom-domain route lets Cloudflare create/manage the DNS record and certificate when deployed.
