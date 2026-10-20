---
date: 2026-10-20
source: cr (CR-0002)
surface: http
outcome: reproduced
runs: "4 of 4 passed, last run 2026-10-20"
---

## Goal

The user sends requests with an API key through Agentloupe and knows that the key is not on disk.

## Preconditions

- The same preconditions as `.agents/scenarios/proxy.stream.md`, with the fake upstream and `loupe serve` running.
- The value of `LOUPE_ACCOUNT_KEY` is known to the tester.

## Steps at the user surface

1. Run `curl -s -X POST -H 'x-api-key: sk-test-123' -H 'chatgpt-account-id: acct-test-9' --data '{"p":"secret-prompt"}' http://127.0.0.1:8787/claude-code/fake/json`.
2. Run `sqlite3 $LOUPE_DATA_DIR/loupe.db "select request_headers, credential_source, credential_hash, account_id_source, account_id_hash from exchanges"`.
3. Compute the expected hash: `echo -n sk-test-123 | openssl dgst -sha256 -mac HMAC -macopt hexkey:$(echo $LOUPE_ACCOUNT_KEY | base64 -d | xxd -p -c64)`.
4. Run `grep -ra -c -e sk-test-123 -e acct-test-9 -e secret-prompt $LOUPE_DATA_DIR`.
5. Stop both processes.

## Success condition

- The stored headers show `x-api-key` and `chatgpt-account-id` with the value `redacted`.
- `credential_source` is `x-api-key` and `credential_hash` equals the hash of step 3.
- Step 4 gives 0 for each file: `loupe.db`, `loupe.db-wal`, and `logs/loupe.log`.

## Run log

- Run 1: headers `redacted`, both hashes equal the openssl values, 0 matches in each file. Passed.
- Runs 2 to 4: 3 rows with `redacted` headers, 0 matches for `sk-test-123` in the data directory. Passed.
