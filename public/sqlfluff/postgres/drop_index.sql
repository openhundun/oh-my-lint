drop index abc;

drop index "abc";

drop index if exists abc;

drop index abc, "def", ghi;

drop index if exists abc, def, ghi;

-- Test CASCADE trailing keyword

drop index abc cascade;

drop index abc, def, ghi cascade;

drop index if exists abc, def, ghi cascade;

-- Test RESTRICT trailing keyword

drop index abc restrict;

drop index abc, def, ghi restrict;

-- Test CONCURRENTLY

DROP INDEX CONCURRENTLY abc;

DROP INDEX CONCURRENTLY IF EXISTS abc;

DROP INDEX CONCURRENTLY abc, def;

DROP INDEX CONCURRENTLY IF EXISTS abc, def;

DROP INDEX CONCURRENTLY abc, def CASCADE;
