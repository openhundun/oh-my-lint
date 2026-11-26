-- MERGE in EXPLAIN, and in COPY with RETURNING (PostgreSQL 17).
-- https://www.postgresql.org/docs/17/sql-explain.html
-- https://www.postgresql.org/docs/17/sql-copy.html

explain merge into t using s on t.id = s.id
when matched then delete;

explain (analyze, costs false)
merge into t using s on t.id = s.id
when matched then update set v = s.v
when not matched then insert(id, v) values (s.id, s.v);

copy(
    merge into t using s on t.id = s.id
    when matched then update set v = s.v
    returning merge_action(), t.id
) to stdout;
