-- @agents-index Adds the API-equivalent cost, the price commit, and the unpriced reason to calls; null until a pricing pass sees the call.
ALTER TABLE calls ADD COLUMN cost_usd TEXT;
ALTER TABLE calls ADD COLUMN price_commit TEXT;
ALTER TABLE calls ADD COLUMN unpriced_reason TEXT;
