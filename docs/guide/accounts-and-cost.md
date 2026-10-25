<!-- @agents-index User guide: How loupe finds the accounts and calculates the cost of the calls. -->

# Accounts and cost

## Accounts

The binary identifies the provider account between sessions without a sign-up. For each
request, it uses the first source that exists:

1. A stable account ID that the agent sends:
   - Codex (ChatGPT login): the `ChatGPT-Account-Id` header.
   - Claude Code: the account UUID in `metadata.user_id` of the request body.
2. The credential that the agent sends to the provider: the `authorization` bearer token or the
   `x-api-key` header. This is the fallback that catches every request without an account ID.

A capture of real requests on 2026-10-20 verified these sources per agent version:

| Agent | Version | Account source | Session source |
|---|---|---|---|
| Claude Code | 2.1.304 | `metadata.user_id`, a JSON object string with `account_uuid`, `device_id`, `session_id` | the header `x-claude-code-session-id`; a subagent call also sends `x-claude-code-agent-id`, which makes it its own session |
| Pi | 1.3.0 | the credential in `authorization` | no session header and no `prompt_cache_key`, so the hash of the opening of the conversation |

The Codex source is not verified yet.

The binary computes an HMAC-SHA256 of the value with the server secret `LOUPE_ACCOUNT_KEY`,
before it redacts the headers. It stores only the hash and the name of the source. It never
stores the raw credential or the raw account ID. `LOUPE_ACCOUNT_KEY` is separate from
`LOUPE_DATA_KEY`, so a rotation of the data key does not change the accounts. Without the server
secret, nobody can compute the hash from a known key, so a copy of the database does not show
which rows belong to which key.

Known limits:

- A subscription login with no account ID in the request falls back to the provider token. That
  token refreshes, so each refresh starts a new account.
- An API key that you rotate starts a new account.
- One person who uses more than one provider gets one account for each provider.
- The account ID sources depend on the request format of each agent, and that format can change.

## Cost

Agentloupe shows what each call costs at the list price of the provider API. It does this also
when the agent uses a subscription. For a subscription, the figure is the API-equivalent cost:
what the same tokens would cost on the API, not what you paid. The dashboard and the API label
it as such.

The prices come from `vendor/litellm/model_prices_and_context_window.json`, the LiteLLM price
file. `vendor/litellm/model_prices_and_context_window.schema.json` is its JSON schema, from the
same commit. `vendor/litellm/SOURCE.md` records the pinned commit and the checksums. Both files
are MIT licensed (`vendor/litellm/LICENSE`). The binary builds the price file in at compile
time.

To refresh the prices, run `.agents/scripts/prices.vendor.sh`, then rebuild the binary. The
script validates the price file against the schema before it writes anything. A test of the
binary validates it again, so a refresh that breaks the format fails the build. The price fields
that the binary reads use the names that the schema defines. The schema uses Python regex
syntax, for example `(?i)`, so the validator must use a Python-compatible regex dialect.

The binary prices each call with these rules:

1. **Find the entry.** Use the model name from the response, and the entry whose
   `litellm_provider` matches the upstream. If no entry matches, try the name without a
   trailing date (`-YYYYMMDD`).
2. **Add the token costs.**
   - Uncached input × `input_cost_per_token`
   - Cache reads × `cache_read_input_token_cost`
   - Cache writes with a 5-minute lifetime × `cache_creation_input_token_cost`
   - Cache writes with a 1-hour lifetime × `cache_creation_input_token_cost_above_1hr`
   - Output, which includes reasoning tokens × `output_cost_per_token`
3. **Apply the prompt-size tier.** The prompt size is uncached input + cache reads + cache
   writes. If it is above the threshold of an `…_above_<N>k_tokens` field, use the price of
   that tier for every token in the call.
4. **Apply the service tier.** If the response reports the `priority` or `flex` service tier,
   use the `…_priority` or `…_flex` field when one exists.
5. **Do not guess.** If no entry matches, or a price field is missing for a token count that is
   not zero, the call has no price. Its cost is `null`, and the call counts in
   `unpriced_calls`.
6. **Keep history stable.** The binary stores the cost on each call, together with the commit
   of the price file. A later refresh of the prices does not change past days.

To apply these rules, the parser must record 1-hour cache writes apart from 5-minute cache
writes, and the service tier of each response.
