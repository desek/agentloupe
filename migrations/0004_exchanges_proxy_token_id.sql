-- @agents-index Adds proxy_token_id to exchanges: the ID of the proxy token that passed the token gate, null in no-token mode.
ALTER TABLE exchanges ADD COLUMN proxy_token_id INTEGER REFERENCES proxy_tokens (id);
