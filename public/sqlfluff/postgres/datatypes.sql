create table a (
    a  smallint,
    b  int,
    ba smallint,
    bb int,
    bc bigint,
    bd int,
    c  bigint,
    d  float4,
    e  float8,
    f  smallserial,
    g  serial,
    ga serial2,
    gb serial4,
    gc serial8,
    h  bigserial
);

create table b (
    a float,
    b float(24),
    c float4,
    e float8
);

create table c (
    a  decimal,
    aa decimal,
    b  decimal(7),
    ba decimal(7),
    c  decimal(7, 2),
    ca decimal(7, 2)
);

create table d (a money);

create table e (
    a char,
    b char(7),
    c char,
    d char(5),
    e varchar,
    f varchar(8),
    g varchar(9),
    h varchar,
    i text
);

create table f (a bytea);

create table g (
    a date,
    b interval(4),
    c time(4),
    d timetz(4),
    e time(4),
    f timestamp(4),
    g timestamptz(4),
    h timestamp(4),
    i timetz,
    j timetz(4),
    k timestamptz,
    l timestamptz(4)
);

create table h (a boolean, b boolean);

create table i (
    a point,
    b line,
    c lseg,
    d box,
    e path,
    f polygon,
    g circle
);

create table j (
    a cidr,
    b inet,
    c macaddr,
    d macaddr8
);

create table k (
    a bit,
    b bit(3),
    c bit varying,
    d bit varying(5)
);

create table l (a pg_lsn);

create table l (a tsvector, b tsquery);

create table m (a uuid);

create table n (a xml);

create table o (a json, b jsonb);

create table p (
    a int[],
    b float[][],
    c char[1],
    d jsonb[3][5],
    e money array,
    f money array[7]
);

-- user defined data types
create type bar as enum('foo', 'bar');

create table q (a bar);

-- data type with schema
create type public.c as enum('foo', 'bar');

create table r (a public.c);

-- DATETIME is a valid datatype, but is not a date_time_identifier; it is only
-- potentially a user-defined type (i.e. a data_type_identifier).
create table a (
    b date,
    c datetime
);

-- from https://github.com/sqlfluff/sqlfluff/issues/2649
select
    b::datetime
from a;

select b, c::date from a;

create table test (situation bpchar(1) default 'A'::bpchar);
