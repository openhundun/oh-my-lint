select
    extract(year from o_orderdate) as o_year,
    sum(case when n2.n_name = 'FRANCE' then l_extendedprice * (1 - l_discount) else 0 end) / sum(l_extendedprice * (1 - l_discount)) as mkt_share
from
    part,
    supplier,
    lineitem,
    orders,
    customer,
    nation n1,
    nation n2,
    region
where 1 = 1
    and p_partkey = l_partkey
    and s_suppkey = l_suppkey
    and l_orderkey = o_orderkey
    and o_custkey = c_custkey
    and c_nationkey = n1.n_nationkey
    and n1.n_regionkey = r_regionkey
    and r_name = 'EUROPE'
    and s_nationkey = n2.n_nationkey
    and p_type = 'ECONOMY ANODIZED STEEL'
    and p_size = 15
    and (o_orderdate between date '1995-01-01' and date '1996-12-31')
group by extract(year from o_orderdate)
order by o_year
