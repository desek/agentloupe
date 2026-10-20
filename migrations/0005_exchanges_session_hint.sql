-- @agents-index Adds the session hint (HMAC of the first session header and its name) and the parse outcome to exchanges.
ALTER TABLE exchanges ADD COLUMN session_hint_hash TEXT;
ALTER TABLE exchanges ADD COLUMN session_hint_source TEXT;
ALTER TABLE exchanges ADD COLUMN parse_outcome TEXT;
