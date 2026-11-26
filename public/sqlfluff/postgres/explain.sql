explain (analyze true, analyse true, verbose true, costs true, settings true, buffers true, wal true, timing true, summary true, format xml)
select 1;

explain (analyze false, analyse false, verbose false, costs false, settings false, buffers false, wal false, timing false, summary false, format xml)
select 1;

explain (analyze, analyse, verbose, costs, settings, buffers, wal, timing, summary, format xml)
select 1;

explain analyze verbose
select 1;

explain analyse verbose
select 1;

explain analyze
select 1;

explain analyse
select 1;

explain (format text)
select 1;

explain (format json)
select 1;

explain (format yaml)
select 1;

explain (analyze true, verbose false, costs true, buffers false, generic_plan, memory true, serialize)
select 1;

explain (serialize none, generic_plan false)
select 1;

explain (analyze, serialize text)
select 1;

explain (analyze, serialize binary, memory)
select 1;

select memory, serialize, generic_plan from explain_tbl;
