create temp table t1 as (select something from t2);

create temp table t1 as (select something from t2);

create temporary table t1 as (select something from t2);

create table t1 as (select something from t2);

create table t1 as (select something from t2);

create table if not exists t1 as (select something from t2);

create table t1 on commit delete rows as (select something from t2);

create table t1 as (select something from t2);

create table t1 on commit drop as (select something from t2);

create table t1 as (select something from t2) with no data;

create table t1 as (select something from t2) with no data;

create table t1 as (select something from t2) with no data;

create table t1 as (select something from t2) with no data;

create unlogged table t1 as (select something from t2);

create global temp table t1 as (select something from t2);

create local temp table t1 as (select something from t2);

create table t1 using method as (select something from t2);

create table t1 as (select something from t2);

create table t1 (c1, c2, c3) as (values ('val1', 'val2', 'val3'), ('val4', 'val5', 'val6'));

create table t1 as (table t2);

create table t1 as (execute func());

create table t1 tablespace ts as (select something from t2);

create table t1 with (val=70) as (select something from t2);

create temp table t1 with (autovacuum_enabled=true, toast_tuple_target=123, vacuum_index_cleanup=false) as (select column_1, column_2, column_3 from tablename);

create temp table a_new_table with (appendoptimized=true, compresstype=zstd) as (
    select column_1, column_2, column_3
    from schema.tablename
    group by 1, 2, 3
);
