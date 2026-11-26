create materialized view my_mat_view as
select a
from my_table;

create materialized view if not exists my_mat_view as
select a
from my_table;

create materialized view my_mat_view as
(
    select a
    from my_table
);

CREATE MATERIALIZED VIEW my_mat_view AS
SELECT a
FROM my_table
WITH NO DATA;

CREATE MATERIALIZED VIEW my_mat_view AS
SELECT a
FROM my_table
WITH DATA;

create materialized view if not exists my_mat_view as
(
    select a
    from my_table
);

create materialized view my_mat_view as
(
    select
        a,
        b
    from
        my_table
    where y = 'value'
);

create materialized view if not exists my_mat_view as
(
    select
        a,
        b
    from
        my_table
    where y = 'value'
);


create materialized view my_mat_view as
select
    a,
    b
from
    my_table
where y = 'value';

create materialized view if not exists my_mat_view as
select
    a,
    b
from
    my_table
where y = 'value';


create materialized view my_mat_view as
select
    a,
    b
from
    my_table;

create materialized view if not exists my_mat_view as
select
    a,
    b
from
    my_table;

-- SQL from issue #2039
CREATE MATERIALIZED VIEW bar
AS
(
    SELECT col
    FROM my_table
)
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS bar
AS
(
    SELECT col
    FROM my_table
)
WITH NO DATA;


CREATE MATERIALIZED VIEW my_mat_view
USING heap
WITH (prop_a = 1, prob_b = 'some_value', prop_c = FALSE, prop_d)
TABLESPACE pg_default
AS
(
    SELECT
        a,
        avg(b) AS my_avg,
        count(*) AS my_count
    FROM my_table
    GROUP BY grp
    HAVING col > 2
)
WITH DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS my_mat_view
USING heap
WITH (prop_a = 1, prob_b = 'some_value', prop_c = FALSE, prop_d)
TABLESPACE pg_default
AS
(
    SELECT
        a,
        avg(b) AS my_avg,
        count(*) AS my_count
    FROM my_table
    GROUP BY grp
    HAVING col > 2
)
WITH DATA;

CREATE MATERIALIZED VIEW my_mat_view
TABLESPACE pg_default
AS
SELECT
    table_1.field_1,
    table_1.field_2
FROM table_1
UNION
SELECT
    table_2.field_1,
    table_2.field_2
FROM table_2

ORDER BY field_1, field_2
WITH DATA;

create materialized view my_mat_view
with (left.right)
as
select a
from my_table;

create materialized view my_mat_view
with (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc)
as
select a
from my_table;

create or replace materialized view my_mat_view as
select a
from my_table;
