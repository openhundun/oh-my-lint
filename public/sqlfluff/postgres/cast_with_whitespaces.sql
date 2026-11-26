-- postgres_cast_with_whitespaces.sql
/* Several valid queries where there is whitespace surrounding the Postgres
cast operator (::) */

-- query from https://github.com/sqlfluff/sqlfluff/issues/2720
select amount_of_honey :: float
from bear_inventory;


-- should be able to support an arbitrary amount of whitespace
select amount_of_honey        ::        float
from bear_inventory;

select amount_of_honey::    float
from bear_inventory;

select amount_of_honey        ::float
from bear_inventory;

-- should support a wide variety of typecasts
select amount_of_honey :: time
from bear_inventory;

select amount_of_honey :: text
from bear_inventory;

select amount_of_honey     :: varchar( 512 )
from bear_inventory;

select amount_of_honey ::        timestamptz
from bear_inventory;

select amount_of_honey    ::        timestamp
from bear_inventory;

-- should support casts with arbitrary amount of whitespace in join statements
select
    bi.amount_of_honey
from bear_inventory bi
left join favorite_cola fc
    on fc.bear_id ::   varchar(512) = bi.bear_id    ::varchar(512)
where fc.favorite_cola = 'RC Cola';
