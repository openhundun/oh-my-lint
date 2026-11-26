-- `#` is the bitwise XOR of integers and bit strings.
select 5 # 3;

select B'10101' # B'01110';

select flags # 4 as toggled
from settings
where (flags # mask) & 1 = 1;

select 5#3;

-- It's also the point where two geometric objects intersect.
select lseg '[(0,0),(2,2)]' # lseg '[(0,2),(2,0)]';

-- `##` is the point on the second object closest to the first.
select point '(1,2)' ## lseg '[(0,0),(2,0)]';

select 5 operator(pg_catalog.#) 3;

-- The JSON operators that start with `#` are still one operator each.
select data #> '{a,b}', data #>> '{a,b}', data #- '{a}'
from documents;
