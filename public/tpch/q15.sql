select
    s_suppkey,
    s_name,
    s_address,
    s_phone,
    total_revenue
from
    supplier,
    (
        select l_suppkey, sum(l_extendedprice * (1 - l_discount)) as total_revenue
        from lineitem
        where l_shipdate >= '1996-01-01'::date and l_shipdate < '1996-04-01'::date
        group by l_suppkey
    ) revenue0
where (
    1 = 1
    and s_suppkey = l_suppkey
    and total_revenue = (
        select max(total_revenue)
        from (
            select l_suppkey, sum(l_extendedprice * (1 - l_discount)) as total_revenue
            from lineitem
            where l_shipdate >= '1996-01-01'::date and l_shipdate < '1996-04-01'::date
            group by l_suppkey
        ) revenue1
    )
)
order by s_suppkey;
