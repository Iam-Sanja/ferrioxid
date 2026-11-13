# Remote storage plan

[Overview](index.md) · Local storage: [storage.md](storage.md)

**Not a client.** `prepare_s3` checks an `https://` endpoint, a region and a bucket name. It does not open a socket. `credentials_from_env` reads the two environment names stored on the plan at call time. The names are `FEXU_S3_ACCESS_KEY` and `FEXU_S3_SECRET_KEY` when the application chooses those constants. Credentials are not compiled into the binary. `Debug` for `S3Credentials` prints `[redacted]`.

There is no success path that pretends an upload reached a bucket. Callers that need bytes today use `LocalStorage`.

Signed downloads are separate from cookies. `sign_download` builds `v1.{owner}.{id}.{exp}.{mac}` with HMAC-SHA256. The key is at least 32 bytes and comes from the caller. A bad MAC is `NotFound`. An expired signature is `Expired`. The signature does not grant another owner's file.

`save_for` and `read_for` require the actor id to equal the owner id. Staff is not implied.

Quotas are best-effort inside one process. `0` means unlimited. A second process does not share the counter.

`cleanup_orphans` removes `*.partial` files, JSON without bytes, bytes without JSON, and expired files. It does not follow symbolic links. `purge_owner` removes one owner's directory. Authentication does not call it, because auth does not depend on storage.
