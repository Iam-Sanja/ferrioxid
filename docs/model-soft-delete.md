# Soft Delete

[Overview](index.md) · Models: [models.md](models.md)

**Open.** v0.26 does not implement soft delete.

An optional `deleted_at` alone would further display deleted lines in lists, REST, admin, foreign keys and bulk actions. At the same time, deletion, restoration, audit, query scopes and unique constraints would have to have the same meaning. This is not complete with today’s `delete_record` without silently changing the existing erasure semantics.

`#[field(managed = "deleted")]` is therefore a compile error. Allowed values are only `"created"` and `"updated"`.

A later building block needs:

- Standard queries without deleted lines
- Delete sets time instead of `DELETE`
- Restore with the same authorization check as the other writing paths
- Audit records that distinguish Delete and Restore
- Unchanged Unique Constraints, expressly documented
- Foreign keys that do not treat deleted targets as existing
