-- https://www.postgresql.org/docs/current/sql-createtype.html
create type foo;
create type bar as enum();
create type bar as enum('foo', 'bar');
create type foobar as range(subtype=float);
CREATE TYPE barbar AS (INPUT = foo, OUTPUT = bar);
create type foofoo as (foo varchar collate utf8);
