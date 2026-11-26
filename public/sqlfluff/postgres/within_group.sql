-- Postgres style WITHIN GROUP window functions
select array_agg(o_orderkey) within group (order by o_orderkey)
from orders
