-- PostgreSQL 17 added `WHEN NOT MATCHED BY SOURCE` and the optional
-- `BY TARGET` qualifier on `WHEN NOT MATCHED`.
-- https://www.postgresql.org/docs/17/sql-merge.html

-- Example adapted from the PostgreSQL 17 MERGE documentation.
merge into wines w
using wine_stock_changes s
on s.winename = w.winename
when not matched by target and s.stock_delta > 0 then
    insert values (s.winename, s.stock_delta)
when matched and w.stock + s.stock_delta > 0 then
    update set stock = w.stock + s.stock_delta
when not matched by source then
    delete;

-- `NOT MATCHED BY SOURCE` combined with `UPDATE` and an extra condition.
merge into target_table t
using source_table s
on t.id = s.id
when matched then
    update set val = s.val
when not matched by source and t.active then
    update set active = false;

-- `BY TARGET` is an accepted synonym for the plain `WHEN NOT MATCHED`.
merge into t
using s
on t.id = s.id
when not matched by target then
    insert(id, val) values (s.id, s.val)
when not matched by source then
    delete;

-- `NOT MATCHED BY SOURCE` also accepts `DO NOTHING`.
merge into t
using s
on t.id = s.id
when not matched by source then
    do nothing;
