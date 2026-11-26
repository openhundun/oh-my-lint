declare c1 cursor for select id, v from t;

declare c2 cursor with hold for select 1;

declare c3 cursor without hold for select id from t order by id;

declare c4 binary insensitive no scroll cursor for select id from t;

declare c5 asensitive scroll cursor for select id from t;

-- The options can come in any order
DECLARE c6 SCROLL BINARY CURSOR FOR SELECT id FROM t;

declare c7 cursor for values (1, 'a'), (2, 'b');

declare c8 cursor for
with x as (select id from t)
select id from x;

declare c9 cursor for select id from t union all select 2;

declare c10 cursor for(select id from t);

-- The query can be in any number of brackets
declare c11 cursor for((select id from t));

declare c12 cursor for(((values (1, 'a'))));

declare c13 cursor for((
    with x as (select id from t)
    select id from x
));

declare c14 cursor for(((select id from t) union (select 2)));
