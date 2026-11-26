select '2005-04-02 12:00:00-07'::timestamptz + '1 day'::interval;

-- DATEADD is not a function in postgres so this should parse day as column name
select dateadd(day, -2, current_date);

select '2013-07-01 12:00:00'::timestamptz - '2013-03-01 12:00:00'::timestamptz;

select 1.0::int;

select '2015-10-24 16:38:46'::timestamp;

select '2015-10-24 16:38:46'::timestamp at time zone 'UTC';

select '2015-10-24 16:38:46'::timestamptz;

select '2015-10-24 16:38:46'::timestamptz at time zone 'UTC';

select '2015-10-24 16:38:46'::timestamp;

select '2015-10-24 16:38:46'::timestamptz;

select '2015-10-24 16:38:46'::timestamptz at time zone 'UTC';

-- Some more example from https://database.guide/how-at-time-zone-works-in-postgresql/

select '2025-11-20 00:00:00+00'::timestamptz at time zone 'Africa/Cairo';

select '2025-11-20 00:00:00'::timestamptz;

select '2025-11-20 00:00:00'::timestamp at time zone 'Africa/Cairo';

select '2025-11-20 00:00:00+12'::timestamp at time zone 'Africa/Cairo';

select '2025-11-20 00:00:00+12'::timestamp;

select '00:00:00+00'::timetz at time zone 'Africa/Cairo';

select '00:00:00'::time at time zone 'Africa/Cairo';

select c_timestamp at time zone 'Africa/Cairo' from t_table;

select (c_timestamp at time zone 'Africa/Cairo')::time from t_table;

select a::float8 from my_table;


select schema1.table1.columna, t.col2
from schema1.table1
cross join lateral somefunc(tb.columnb) t(col1 text, col2 bool);

select a collate "de_DE" < b from test1;

select a < ('foo' collate "fr_FR") from test1;

select a < b collate "de_DE" from test1;

select a collate "de_DE" < b from test1;

select * from test1 order by a || b collate "fr_FR";

-- Select elements are optional in Postgres
select from test1;

-- keywords can be used as column names without quotes if qualified
select id, start, periods.end from periods;

select concat_lower_or_upper('Hello', 'World', true);
select concat_lower_or_upper(a => 'Hello', b => 'World');
select concat_lower_or_upper('Hello', 'World', uppercase => true);

-- row-level locks can be used in Selects
select * from mytable for update;
select * from (select * from mytable for update) ss where col1 = 5;

SELECT col1, col2
FROM mytable1
JOIN mytable2 ON col1 = col2
ORDER BY sync_time ASC
LIMIT 1
FOR SHARE OF mytable1, mytable2 SKIP LOCKED
;

select * from foo tablesample system(10);

select * from foo tablesample bernoulli(10);

-- use of dollar quote in query
select * from (select * from mytable for update) ss where col1 = $1;

select i + $1 into j from foo;

select 1 /* hi hi /* foo */ ho ho */ as bar;

-- escape double quotes
select """t".col1 from tbl1 """t";

select film_id, title from
    film
order by title
fetch first 10 row only;

select foo from bar limit 1 for update;

-- Verify that single underscores are parsed as numeric literals
select 1_000;
select 1_000_000;
select 1.0_000;
select 1_000_000.0_000_000;
