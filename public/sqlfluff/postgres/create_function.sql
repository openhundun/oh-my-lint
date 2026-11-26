-- Some more complicated Postgres function creations.

create function add(integer, integer) returns integer
    as 'select $1 + $2;'
    language sql;

-- Quoted language options are deprecated but still supported
create function add(integer, integer) returns integer
    as 'select $1 + $2;'
    language 'sql';

create or replace function increment(i integer) returns integer as '
    BEGIN
        RETURN i + 1;
    END;
' language plpgsql volatile;

CREATE OR REPLACE FUNCTION increment(i integer) RETURNS integer AS '
    BEGIN
        RETURN i + 1;
    END;
' LANGUAGE plpgsql WINDOW IMMUTABLE STABLE LEAKPROOF RETURNS NULL ON NULL INPUT EXTERNAL SECURITY DEFINER
ROWS 5 SET test_param = 3;

CREATE OR REPLACE FUNCTION increment(i integer) RETURNS integer AS 'C:\\my_file.c', 'symlink_c'
LANGUAGE plpgsql WINDOW IMMUTABLE STABLE NOT LEAKPROOF CALLED ON NULL INPUT EXTERNAL SECURITY DEFINER COST 123
ROWS 5 SET test_param = 3 WITH (isStrict);

CREATE OR REPLACE FUNCTION increment(i integer) RETURNS integer PARALLEL UNSAFE AS $$
    BEGIN
        RETURN i + 1;
    END;
$$ LANGUAGE plpgsql SUPPORT my_function;

create function add(integer, integer) returns integer
    as 'select $1 + $2;'
    language sql
    immutable
    returns null on null input;

create or replace function increment(i integer) returns integer as $$
        BEGIN
                RETURN i + 1;
        END;
$$ language plpgsql;


create function dup(in int, out f1 int, out f2 text)
    as $$ SELECT $1, CAST($1 AS text) || ' is text' $$
    language sql;

select * from dup(42);


create type dup_result as (f1 int, f2 text);

create function dup(int) returns dup_result
    as $$ SELECT $1, CAST($1 AS text) || ' is text' $$
    language sql;

select * from dup(42);

create function dup(int) returns table(f1 int, f2 text)
    as $$ SELECT $1, CAST($1 AS text) || ' is text' $$
    language sql;

create function dup(int) returns table("f1" int, "f2" text)
    as $$ SELECT $1, CAST($1 AS text) || ' is text' $$
    language sql;


select * from dup(42);

create function check_password(uname text, pass text)
returns boolean as $$
DECLARE passed BOOLEAN;
BEGIN
        SELECT  (pwd = $2) INTO passed
        FROM    pwds
        WHERE   username = $1;

        RETURN passed;
END;
$$  language plpgsql
    security definer
    set search_path = admin, pg_temp;


begin;
create function check_password(uname text, pass text)
returns boolean as $$
DECLARE passed BOOLEAN;
BEGIN
        SELECT  (pwd = $2) INTO passed
        FROM    pwds
        WHERE   username = $1;

        RETURN passed;
END;
$$  language plpgsql
    security definer;
revoke all on function check_password(uname text, pass text) from public;
grant execute on function check_password(uname text, pass text) to admins;
commit;

create or replace function public.setof_test()
returns setof text
language sql
stable strict
as $function$
select unnest(array['hi', 'test'])
$function$
;

create or replace function public.foo(_a text, _$b int)
returns float as
$$
  RETURN 0.0
$$ language plpgsql stable parallel safe;

create function _add(integer, integer) returns integer
    as 'select $1 + $2;'
    language sql;

create function _$add(integer, integer) returns integer
    as 'select $1 + $2;'
    language sql;

create function test2(
  x date = current_date
)
returns date
as $$
  begin
    return x;
  end;
$$;

create function test3(
  x date default current_date
)
returns date
as $$
  begin
    return x;
  end;
$$;

CREATE OR REPLACE FUNCTION data_wrapper()
RETURNS SETOF data
STABLE PARALLEL SAFE LEAKPROOF
BEGIN ATOMIC
  SELECT *
  FROM data;
end;

create or replace function union_things()
returns table (num integer)
language sql
stable
begin atomic
  select 1

  union all

  select 2;
end;

create or replace function tz_date(timestamp with time zone, text) returns date
    language sql
    immutable strict
    return($1 at time zone $2)::date;

CREATE FUNCTION storage.insert_dimension
(in_ordinality int, in_fieldname varchar, in_default_val varchar,
 in_valid_from timestamp, in_valid_until timestamp)
returns storage.dimensions language sql
BEGIN ATOMIC
    UPDATE storage.dimensions
       SET ordinality = ordinality + 1
     WHERE ordinality >= in_ordinality;

    insert into storage.dimensions
                (ordinality, fieldname, default_val, valid_from, valid_until)
         values (in_ordinality, in_fieldname,
                coalesce(in_default_val, 'notexist'),
                coalesce(in_valid_from, '-infinity'),
                coalesce(in_valid_until, 'infinity'))
    returning *;
end;

CREATE OR REPLACE FUNCTION time_bucket(
    _time timestamp without time zone,
    _from timestamp without time zone,
    _to timestamp without time zone,
    _buckets integer DEFAULT 200,
    _offset integer DEFAULT 0
)
RETURNS timestamp without time zone
IMMUTABLE PARALLEL SAFE
BEGIN ATOMIC
SELECT date_bin(((_to - _from) / greatest((_buckets - 1), 1)), _time, _from) + ((_to - _from) / greatest((_buckets - 1), 1)) * (_offset + 1);
end;

CREATE OR REPLACE FUNCTION time_bucket_limited(_time timestamp, _from timestamp, _to timestamp, _buckets int = 200)
    RETURNS timestamp
    IMMUTABLE PARALLEL SAFE
BEGIN ATOMIC
    RETURN CASE WHEN _time <= _from THEN _from
      WHEN _time >= _to THEN _to
      ELSE DATE_BIN((_to - _from) / GREATEST(_buckets - 1, 1), _time, _from) + ((_to - _from) / GREATEST(_buckets - 1, 1))
    end;
end;

CREATE OR REPLACE FUNCTION time_series(
    _from timestamp without time zone,
    _to timestamp without time zone,
    _buckets integer DEFAULT 200
)
RETURNS TABLE ("time" timestamp without time zone)
IMMUTABLE PARALLEL SAFE
BEGIN ATOMIC
-- ATTENTION: use integer to generate series, since with timestamps there are rounding issues
SELECT time_bucket(_from, _from, _to, _buckets, g.ofs - 1)
FROM generate_series(0, greatest((_buckets - 1), 1)) AS g (ofs);
end;

create or replace function test (
  _pt geography(point)
)
returns numeric
language sql
begin atomic
  select st_x(_pt::geometry);
end;

CREATE OR REPLACE FUNCTION time_series(
    _from time_table.time_field % TYPE,
    _to time_table.time_field % TYPE,
    _buckets integer DEFAULT 200
)
RETURNS TABLE (pp time_table.time_field % TYPE)
IMMUTABLE PARALLEL SAFE
BEGIN ATOMIC
-- ATTENTION: use integer to generate series, since with timestamps there are rounding issues
SELECT time_bucket(_from, _from, _to, _buckets, g.ofs - 1)
FROM generate_series(0, greatest((_buckets - 1), 1)) AS g (ofs);
end;
