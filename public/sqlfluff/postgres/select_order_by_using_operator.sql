-- PostgreSQL allows an explicit sort operator via `USING operator`
-- instead of ASC/DESC.
-- https://www.postgresql.org/docs/current/queries-order.html
select * from t order by a using <;

select * from t order by a using >;

select *
from t
order by a using < nulls first, b using >;

select * from t order by a using operator (pg_catalog. < );

select * from t order by a using operator (myschema.<<<);
