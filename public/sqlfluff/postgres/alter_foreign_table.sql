alter foreign table distributors alter column street set not null;

alter foreign table t_user add column my_column text;

alter table bar_fdw.foo add test varchar;

alter foreign table myschema.distributors options (add opt1 'value', set opt2 'value2', drop opt3);

ALTER FOREIGN TABLE test OPTIONS (SET table $$(select my_column from my_table)$$);

ALTER FOREIGN TABLE test ADD COLUMN new_column int8, OPTIONS (SET table $$(select my_column from my_table)$$);

alter foreign table test rename to test_renamed;

alter foreign table distributors alter column street options (add opt1 'value', set opt2 'value2', drop opt3);

alter foreign table distributors alter street options (add opt1 'value');

alter foreign table measurements options (set null 'NA', drop "header");
