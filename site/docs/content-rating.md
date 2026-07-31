<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>

# Content Rating System

> **Legacy backend surface.** Rating storage and the developer/marketplace
> API below live in the pre-redesign REST backend (`backend/`) — see
> [Self-hosting](#self-hosting).

Every game on Magnetite carries a content rating that indicates the appropriate age group for its content.  Ratings are set by the developer at game creation and can be updated later.

## Allowed Ratings

| Rating | Label | Description |
|--------|-------|-------------|
| `everyone` | Everyone | Suitable for all ages. No mature themes. |
| `teen` | Teen (13+) | May contain mild violence, suggestive themes, or language. |
| `mature` | Mature (17+) | May contain intense violence, strong language, or adult themes. |

## Setting the Rating

When creating a game via the GDS scaffold flow or the developer API, the `content_rating` field is optional and defaults to `"everyone"`:

```bash
POST /api/v1/developer/games/scaffold
{
  "name": "my-shooter",
  "template_id": "fps",
  "content_rating": "teen"
}
```

Updating an existing game:

```bash
PUT /api/v1/games/:id
{
  "content_rating": "mature"
}
```

Any value outside the three allowed strings (`everyone`, `teen`, `mature`) returns a `422 Unprocessable Entity` with an error message listing the valid options.

## Validation

Backend validation is performed in `backend/src/api/games.rs::validate_content_rating`.  Invalid ratings are rejected before the INSERT/UPDATE reaches the database.

## Display

The Marketplace and GameDetail pages display the rating badge.  The `en.json` i18n file (`src/i18n/en.json`) contains human-readable labels under `games.contentRatings.*`.

## Age Gate (Roadmap)

An optional age-confirmation modal for `mature` games is a planned feature.  When implemented, the play page will check the user's birthdate (collected at registration) against the game's rating and prompt unconfirmed users.  See `docs/requirements.md` for the roadmap item.
