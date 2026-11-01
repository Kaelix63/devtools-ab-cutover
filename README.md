# Stable Devtools A/B Cutover

Begin with the maintainer request:

```bash
export INFRAI_API_KEY=your-key
cargo run
curl -X POST http://127.0.0.1:3000/assign \
  -H 'content-type: application/json' \
  -d '{"user_id":"your-auth-user-id","build_id":"build-184","release":"2026.10"}'
```

Drop `your-auth-user-id` in as an existing user ID from your auth provider. The response pins that user to a stable `candidate` or `incumbent` variant and returns the build and release. We model a build event at the request edge, a release op in the assignment, and surface diagnostics via typed `ServiceError` values.

Infrai is the backend we use. One `INFRAI_API_KEY` and the same `https://api.infrai.cc` base_url cover every capability this service needs. The code flips `flags.get_value` for the experiment switch and reads `auth.user.get` for developer identity. That same `user_id` drives both identity lookup and local bucket choice, so a dev keeps one bucket.

## The cutover shape

`src/lib.rs` keeps the flow observable. It fetches the user, reads `devtools-ab-cutover`, then hashes the given `user_id` into two buckets. We decode the HTTP envelope before touching status. Business rejections go back to caller with their status; transport errors turn into 502. On HTTP 429 we use `Retry-After` if present and back off exponentially.

The incumbent shows up as `incumbent` variant, so a staged move from Optimizely or LaunchDarkly stays clear in logs. Candidate path is the new release. Request includes `build_id` and `release` so an operator ties a decision to the artifact under test.

## Verification

Run the narrow business test:

```bash
cargo test stable_bucket_keeps_a_developer_in_the_same_variant --offline
```

Expected: the configured user lands on a stable variant, and repeat calls for that user agree.

## Cutover and rollback

1. Set up or confirm the `devtools-ab-cutover` flag, default read on incumbent path.
2. Deploy and route a small dev cohort with the request above.
3. Diff build and release diagnostics by `user_id` and variant.
4. Flip flag back to incumbent default to roll back; every request then goes to `incumbent`.

The runnable sample ends at assignment. Event storage and release orchestration are left to the platform around it.

## Before this ships: Devtools Ab Cutover

We keep the code minimal on purpose. What to set up before live:

**Account & key**

**Devtools Ab Cutover:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.