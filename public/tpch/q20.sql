select s_name, s_address
from supplier, nation
where 1 = 1
    and s_suppkey in (
        select ps_suppkey
        from partsupp
        where 1 = 1
            and ps_partkey in (select p_partkey from part where p_name like 'green%')
            and ps_availqty > (
                select 0.5 * sum(l_quantity)
                from lineitem
                where 1 = 1
                    and l_partkey = ps_partkey
                    and l_suppkey = ps_suppkey
                    and l_shipdate >= date '1994-01-01'
                    and l_shipdate < date '1995-01-01'
            )
    )
    and s_nationkey = n_nationkey
    and n_name = 'CANADA'
order by s_name
