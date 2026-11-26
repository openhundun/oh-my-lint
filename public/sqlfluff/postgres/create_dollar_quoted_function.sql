create function foo(integer, integer) returns integer
    as $$ select $1 + $2; $$
    language sql;
