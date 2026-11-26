select tbl1.column1, tbl2.column1, tbl1.column2 && tbl2.column2 as overlap
from ( values
	(1, 'LINESTRING(0 0, 3 3)'::geometry),
	(2, 'LINESTRING(0 1, 0 5)'::geometry)) tbl1,
( values
	(3, 'LINESTRING(1 2, 4 6)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 &&& tbl2.column2 as overlaps_3d,
			            tbl1.column2 && tbl2.column2 as overlaps_2d
from ( values
	(1, 'LINESTRING Z(0 0 1, 3 3 2)'::geometry),
	(2, 'LINESTRING Z(1 2 0, 0 5 -1)'::geometry)) tbl1,
( values
	(3, 'LINESTRING Z(1 2 1, 4 6 1)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 &< tbl2.column2 as overleft
from
  ( values
	(1, 'LINESTRING(1 2, 4 6)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING(0 0, 3 3)'::geometry),
	(3, 'LINESTRING(0 1, 0 5)'::geometry),
	(4, 'LINESTRING(6 0, 6 1)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 &<| tbl2.column2 as overbelow
from
  ( values
	(1, 'LINESTRING(6 0, 6 4)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING(0 0, 3 3)'::geometry),
	(3, 'LINESTRING(0 1, 0 5)'::geometry),
	(4, 'LINESTRING(1 2, 4 6)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 &> tbl2.column2 as overright
from
  ( values
	(1, 'LINESTRING(1 2, 4 6)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING(0 0, 3 3)'::geometry),
	(3, 'LINESTRING(0 1, 0 5)'::geometry),
	(4, 'LINESTRING(6 0, 6 1)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 << tbl2.column2 as strict_left
from
  ( values
	(1, 'LINESTRING (1 2, 1 5)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING (0 0, 4 3)'::geometry),
	(3, 'LINESTRING (6 0, 6 5)'::geometry),
	(4, 'LINESTRING (2 2, 5 6)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 <<| tbl2.column2 as below
from
  ( values
	(1, 'LINESTRING (0 0, 4 3)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING (1 4, 1 7)'::geometry),
	(3, 'LINESTRING (6 1, 6 5)'::geometry),
	(4, 'LINESTRING (2 3, 5 6)'::geometry)) tbl2;

select 'LINESTRING(0 0, 0 1, 1 0)'::geometry = 'LINESTRING(1 1, 0 0)'::geometry;

select tbl1.column1, tbl2.column1, tbl1.column2 >> tbl2.column2 as strict_right
from
  ( values
	(1, 'LINESTRING (2 3, 5 6)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING (1 4, 1 7)'::geometry),
	(3, 'LINESTRING (6 1, 6 5)'::geometry),
	(4, 'LINESTRING (0 0, 4 3)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 @ tbl2.column2 as contained
from
  ( values
	(1, 'LINESTRING (1 1, 3 3)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING (0 0, 4 4)'::geometry),
	(3, 'LINESTRING (2 2, 4 4)'::geometry),
	(4, 'LINESTRING (1 1, 3 3)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 |&> tbl2.column2 as overabove
from
  ( values
	(1, 'LINESTRING(6 0, 6 4)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING(0 0, 3 3)'::geometry),
	(3, 'LINESTRING(0 1, 0 5)'::geometry),
	(4, 'LINESTRING(1 2, 4 6)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 |>> tbl2.column2 as above
from
  ( values
	(1, 'LINESTRING (1 4, 1 7)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING (0 0, 4 2)'::geometry),
	(3, 'LINESTRING (6 1, 6 5)'::geometry),
	(4, 'LINESTRING (2 3, 5 6)'::geometry)) tbl2;

select tbl1.column1, tbl2.column1, tbl1.column2 ~ tbl2.column2 as contains
from
  ( values
	(1, 'LINESTRING (0 0, 3 3)'::geometry)) tbl1,
  ( values
	(2, 'LINESTRING (0 0, 4 4)'::geometry),
	(3, 'LINESTRING (1 1, 2 2)'::geometry),
	(4, 'LINESTRING (0 0, 3 3)'::geometry)) tbl2;

select 'LINESTRING(0 0, 1 1)'::geometry ~= 'LINESTRING(0 1, 1 0)'::geometry as equality;

select st_distance(geom, 'SRID=3005;POINT(1011102 450541)'::geometry) as d, edabbr, vaabbr
from va2005
order by geom <-> 'SRID=3005;POINT(1011102 450541)'::geometry limit 10;

select track_id, dist from (
  select track_id, st_distancecpa(tr, :qt) as dist
  from trajectories
  order by tr |=| :qt
  limit 5
) foo;

select *
from (
select b.tlid, b.mtfcc,
	b.geom <#> st_geomfromtext('LINESTRING(746149 2948672,745954 2948576,
		745787 2948499,745740 2948468,745712 2948438,
		745690 2948384,745677 2948319)', 2249) as b_dist,
		st_distance(b.geom, st_geomfromtext('LINESTRING(746149 2948672,745954 2948576,
		745787 2948499,745740 2948468,745712 2948438,
		745690 2948384,745677 2948319)', 2249)) as act_dist
    from bos_roads b
    order by b_dist, b.tlid
    limit 100) foo
    order by act_dist, tlid
limit 10;

with
index_query as (
  select st_distance(geom, 'SRID=3005;POINT(1011102 450541)'::geometry) as d, edabbr, vaabbr
	from va2005
  order by geom <<->> 'SRID=3005;POINT(1011102 450541)'::geometry limit 100
)

select *
	from index_query
  order by d limit 10;

with
index_query as (
  select st_distance(geom, 'SRID=3005;POINT(1011102 450541)'::geometry) as d, edabbr, vaabbr
	from va2005
  order by geom <<#>> 'SRID=3005;POINT(1011102 450541)'::geometry limit 100
)

select *
	from index_query
  order by d limit 10;
