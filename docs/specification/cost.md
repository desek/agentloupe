<!-- @agents-index Specification of the API-equivalent cost from the embedded price file, the unpriced reasons, and the subscription cost per day. -->
# Cost

The API-equivalent cost of a call is its cost at the list price of the provider API. The price comes from the vendored LiteLLM price file in `vendor/litellm/`. The cost is the same when the agent uses a subscription. The real price of a subscription is a different figure. See "Subscription cost".

## Embedding and the price commit

The build embeds `vendor/litellm/model_prices_and_context_window.json` in the binary. The binary does not read the price file from the file system at run time.

The build also embeds the price commit. The price commit is the value of the `Commit` row of `vendor/litellm/SOURCE.md`, 40 lowercase hex characters. If this row is missing or not valid, the build stops with an actionable error. The error names `.agents/scripts/prices.vendor.sh` as the fix. A change to a file in `vendor/litellm/` makes the build run again.

At start, `loupe serve` parses the embedded file one time into a price index. The key of the index is the model name and the provider. The entry `sample_spec` is not a model, and the index skips it.

## Provider of an upstream

Each entry of `upstreams.toml` names a LiteLLM provider. See `proxy.md`. The shipped file maps `anthropic` to `anthropic`, and `openai` and `chatgpt` to `openai`. Thus a call through `chatgpt` gets the price of the OpenAI API.

## Lookup

The pricing function looks up the model of the call with the provider of its upstream. If no entry matches, it removes one trailing date suffix from the model name and looks up again. A date suffix is a hyphen and eight digits, for example `-20250929`.

The function never matches an entry of a different provider. It never matches by prefix or by similarity.

## Token terms

The cost is the sum of five terms:

| Token count | Base price field |
|---|---|
| `input` | `input_cost_per_token` |
| `cache_read` | `cache_read_input_token_cost` |
| `cache_write_5m` | `cache_creation_input_token_cost` |
| `cache_write_1h` | `cache_creation_input_token_cost_above_1hr` |
| `output` | `output_cost_per_token` |

Each term is the token count multiplied by its price field. The `output` count already holds the reasoning tokens. The function does not add them a second time.

## Prompt-size tiers

The prompt size is `input` plus `cache_read` plus `cache_write_5m` plus `cache_write_1h`.

A price field with the suffix `_above_<N>k_tokens` is a tier field. Its threshold is N multiplied by 1000 tokens. For each term, the function uses the tier field with the highest threshold that the prompt size is above. If the prompt size is above no threshold, the term uses its base field. A term with no tier field always uses its base field.

## Service tiers

If the service tier of the call is `priority`, each term uses its field with the suffix `_priority`. If the service tier is `flex`, each term uses its field with the suffix `_flex`. A term with no such field uses its base field. Any other service tier uses the base fields.

## Order when both tiers apply

When a service tier and a prompt-size tier both apply, the function selects the service tier field first. Then it uses the prompt-size tier of that field if the entry has one. If the entry has none, it uses the service tier field.

## Unpriced calls

The function does not guess a price. A call is unpriced in these two cases:

| Case | `unpriced_reason` |
|---|---|
| No entry matches. | `no_entry` |
| A price field is missing for a token count that is not zero. | `missing_field:<field name>` |

The cost of an unpriced call is null. A missing field for a token count of zero does not make the call unpriced.

Each unpriced call writes one log line at level `warn`. The line holds the call id, the model, the upstream, and the reason. It gives the fix: run `.agents/scripts/prices.vendor.sh` and rebuild. It also states that new prices apply only to new calls and to calls with a null `price_commit`.

## Arithmetic

The function uses exact decimal arithmetic, not binary floating point. It stores the cost in US dollars as decimal text with at most 12 fraction digits. Thus a sum of many calls does not drift.

The function is pure. It reads only the call and the price index, and it does no input or output.

## Columns of a call

The parser worker writes three columns of `calls` in the same transaction as the call row:

| Column | Value |
|---|---|
| `cost_usd` | The cost as decimal text, or null for an unpriced call. |
| `price_commit` | The price commit that priced the call. It is set for a priced call and for an unpriced call. It is null only for a call that no pricing pass saw. |
| `unpriced_reason` | The reason, or null for a priced call. |

A call that has a `price_commit` keeps its cost. No command reprices it. Thus a refresh of the price file does not change the history.

## `loupe price backfill`

The subcommand prices only the calls with a null `price_commit`. It prints these lines on stdout and exits with code 0:

```
calls_priced: <count>
calls_unpriced: <count>
price_commit: <commit>
```

You can run it again safely. Under `--dry-run`, it prints the counts and writes nothing.

## `loupe price show`

`loupe price show --model <name> --upstream <name>` prints the matched entry key, the provider, and the price fields that the function reads, as `key: value` lines. If no entry matches, it prints `match: none` and exits with code 0. It reads only the embedded index.

## `loupe config show`

The subcommand prints `price_commit` and the number of entries in the price index. It also prints the subscription lines. See "Subscription cost".

## Validation

A test validates the embedded price file against the embedded schema. The validator accepts the inline flag `(?i)` in a pattern. If a pattern does not compile, the test fails with an actionable error that names the pattern.

A test fails if a price field that the function reads is not a property of the schema.

## Required model

The pinned price file must hold the model `claude-haiku-5-5` with the provider `anthropic`. The entry must hold these fields: `input_cost_per_token`, `output_cost_per_token`, `cache_read_input_token_cost`, `cache_creation_input_token_cost`, and `cache_creation_input_token_cost_above_1hr`. A tier field is required only if the upstream entry has it.

A test fails with an actionable error if the embedded file does not meet this rule. The error names the model, the missing field, and the fix: `.agents/scripts/prices.vendor.sh --ref <commit>` with a commit that holds the entry. `prices.vendor.sh` does the same check after the schema validation. If the check fails, the script writes nothing, prints an actionable error on stderr, and exits with code 4.

## Refresh path

Do not edit the files in `vendor/litellm/` by hand. To refresh the prices:

1. Run `.agents/scripts/prices.vendor.sh`.
2. Rebuild the binary.
3. Run `.agents/scripts/pipeline.verdict.sh`.

The new prices apply to new calls. Run `loupe price backfill` to price the calls with a null `price_commit`.

## Subscription cost

The subscription cost is the real price that the user pays for a subscription. It is a different figure from the API-equivalent cost.

Each variable `LOUPE_SUBSCRIPTION_<NAME>=<USD per month>` declares one subscription. These rules apply:

1. `<NAME>` matches `[A-Z0-9_]+`. The subscription name is `<NAME>` in lowercase. For example, `LOUPE_SUBSCRIPTION_CLAUDE_MAX=200` declares `claude_max` at 200 US dollars per month.
2. The value is a non-negative decimal with at most 2 fraction digits.
3. No variable means no subscription and a subscription cost of 0. No subscription is a default.
4. The order of precedence is the order of `configuration.md`.

If a name or a value is not valid, `loupe serve` stops before it opens the port. The actionable error names the variable, the expected shape, and the check, `loupe config show`.

`loupe config show` prints each subscription as `subscription.<name>: <USD per month>`. It also prints `subscription_total_per_month: <sum>`.

### Pro-rating per day

A pure function gives the subscription cost of one calendar day in `LOUPE_TZ`. For each subscription, it divides the monthly price by the number of days of the calendar month of that day. It returns the sum over all subscriptions as decimal text with at most 12 fraction digits, in exact decimal arithmetic.

The subscription cost of a range is the sum of this function over each day of the range. The function uses the current configuration for every day, past or future, because the configuration holds no history of prices. It also gives the cost of a day with no calls.
