select o_orderpriority, count(*) as order_count
from orders
where (
    1 = 1
    and o_orderdate >= '1993-07-01'::date
    and o_orderdate < '1993-10-01'::date
    and exists (
        select 1
        from lineitem
        where l_orderkey = o_orderkey and l_commitdate < l_receiptdate
    )
)
group by o_orderpriority
order by o_orderpriority;
