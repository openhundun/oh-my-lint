-- The TABLE command, short for SELECT * FROM
TABLE t;

TABLE ONLY t;

TABLE public.t *;

TABLE t ORDER BY id LIMIT 5;

TABLE t OFFSET 2 FETCH FIRST 3 ROWS ONLY;

TABLE t FOR UPDATE;

-- In set expressions, ORDER BY and LIMIT belong to the set expression
TABLE a UNION TABLE b ORDER BY 1 LIMIT 5;

(table a) union all (table b);

-- Wherever a SELECT can be used
with x as (table t) table x;

select * from (table t) sub;

INSERT INTO t2 TABLE t;

create view v as table t;
