select
    region,
    city,
    grouping(region, city) as grp_idx,
    count(distinct id) as num_total,
    count(distinct id) filter(where is_poi) as num_poi,
    count(distinct id) filter(where is_gov) as num_gov
from location_data
group by grouping sets( (region), (city), (region, city), () );

select
    region,
    city,
    grouping(region, city) as grp_idx,
    count(distinct id) as num_total,
    count(distinct id) filter(where is_poi) as num_poi,
    count(distinct id) filter(where is_gov) as num_gov
from location_data
group by rollup( (region), (city) );

select
    region,
    city,
    grouping(region, city) as grp_idx,
    count(distinct id) as num_total,
    count(distinct id) filter(where is_poi) as num_poi,
    count(distinct id) filter(where is_gov) as num_gov
from location_data
group by cube( (region), (city) );

-- GROUP BY DISTINCT / ALL (PostgreSQL 14+)
SELECT a, b, c, count(*)
FROM tbl
GROUP BY DISTINCT ROLLUP (a, b), ROLLUP (a, c);

SELECT a, b, count(*)
FROM tbl
GROUP BY ALL CUBE (a, b);

SELECT a, count(*)
FROM tbl
GROUP BY DISTINCT a;
