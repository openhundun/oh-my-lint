with
w as materialized (
    select * from other_table
)
select *
from w w1
join w w2 on w1.key = w2.ref
where w2.key = 123;

with
w as not materialized (
    select * from big_table
)
select *
from w w1
join w w2 on w1.key = w2.ref
where w2.key = 123;

WITH RECURSIVE search_tree(id, link, data) AS (
    SELECT t.id, t.link, t.data
    FROM tree t
  UNION ALL
    SELECT t.id, t.link, t.data
    FROM tree t, search_tree st
    WHERE t.id = st.link
) SEARCH DEPTH FIRST BY id SET ordercol
SELECT * FROM search_tree ORDER BY ordercol;

WITH RECURSIVE search_tree(id, link, data) AS (
    SELECT t.id, t.link, t.data
    FROM tree t
  UNION ALL
    SELECT t.id, t.link, t.data
    FROM tree t, search_tree st
    WHERE t.id = st.link
) SEARCH BREADTH FIRST BY id SET ordercol
SELECT * FROM search_tree ORDER BY ordercol;

WITH RECURSIVE search_graph(id, link, data, depth) AS (
    SELECT g.id, g.link, g.data, 1
    FROM graph g
  UNION ALL
    SELECT g.id, g.link, g.data, sg.depth + 1
    FROM graph g, search_graph sg
    WHERE g.id = sg.link
) CYCLE id SET is_cycle USING path
SELECT * FROM search_graph;

-- test that DML queries are also selectable
with tbl as (
    insert into a values (5) returning *
)
select * from tbl;

with
tbl as (
    update a set b = 5 returning *
)
select *
from tbl;

with
tbl as (
    delete from a returning *
)
select *
from tbl;
