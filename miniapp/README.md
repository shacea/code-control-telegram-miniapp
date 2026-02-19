# cokacdir Mini App (Option A)

Static Telegram Mini App used only for **session start UX**:

- Choose engine: `opencode` or `claude`
- Choose folder under `~/Projects`
- Sends the selection back via `Telegram.WebApp.sendData()`

## Deploy on Cloudflare Pages

1) Create a Cloudflare Pages project connected to this repo.
2) Root directory: `miniapp/`
3) Build command: (none)
4) Output directory: `.`

## Configure bot

Set env var when running the ccserver:

```bash
export COKACDIR_MINIAPP_URL="https://<your-project>.pages.dev"
```

Then in Telegram:

- `/app` → opens the Mini App

## Security

Bot only provides folder names from `~/Projects`.
Bot validates returned folder resolves to a directory within `~/Projects`.
