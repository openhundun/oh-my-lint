truncate bigtable;
truncate some_schema.bigtable;
truncate table bigtable;
truncate only bigtable;
truncate table only bigtable;
truncate bigtable *;
truncate table bigtable *;
truncate bigtable, fattable;
truncate table bigtable, fattable;
truncate only bigtable, fattable *;
truncate bigtable restart identity;
truncate bigtable continue identity;
truncate bigtable cascade;
truncate bigtable restrict;
truncate table
    only bigtable,
    fattable *,
    only slimtable
continue identity cascade;
