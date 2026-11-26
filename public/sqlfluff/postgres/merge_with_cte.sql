-- A WITH clause before MERGE, and MERGE as a data-modifying statement inside
-- a WITH clause (the latter needs RETURNING, added in PostgreSQL 17).
-- https://www.postgresql.org/docs/17/sql-merge.html
-- https://www.postgresql.org/docs/17/queries-with.html#QUERIES-WITH-MODIFYING

with src as (
    select id, v from s where id > 0
)
merge into t using src on t.id = src.id
when matched then update set v = src.v
when not matched then insert(id, v) values (src.id, src.v);

with a as (select id, v from s), b as (select id, v from a where id > 1)
merge into t using b on t.id = b.id
when not matched then do nothing;

with src as materialized (select id from s)
merge into t as tgt using src on tgt.id = src.id
when matched then delete;

with src as (select id, v from s)
merge into t using src on t.id = src.id
when matched then update set v = src.v
returning merge_action(), t.*;

with
merged as (
    merge into t using s on t.id = s.id
    when matched then update set v = s.v
    when not matched then insert(id, v) values (s.id, s.v)
    returning merge_action() as action, t.id
)
select action, count(*)
from merged
group by action;
