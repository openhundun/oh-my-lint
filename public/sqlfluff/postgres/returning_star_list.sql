-- `*` can be one item of a RETURNING list, next to other output expressions.
-- https://www.postgresql.org/docs/17/sql-insert.html
-- https://www.postgresql.org/docs/17/sql-merge.html

insert into t (id, v) values (5, 'x') returning *, id + 1 as next_id;

update t set v = 'y' where id = 1 returning id, *;

delete from t where id = 3 returning *, now();

merge into t using s on t.id = s.id
when matched then update set v = s.v
returning merge_action(), *;

merge into t using s on t.id = s.id
when not matched then insert(id, v) values (s.id, s.v)
returning *, merge_action() as action;

-- PostgreSQL 18 aliases for the old and new rows.
UPDATE t SET v = 'y' WHERE id = 1
RETURNING WITH (OLD AS o) o.v AS old_v, *;
