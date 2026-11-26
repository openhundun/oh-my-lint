create unique index title_idx on films (title);

create unique index title_idx on films (title) include (director, rating);

create index title_idx on films (title) with (deduplicate_items='off');

create index on films ((lower(title)));

create index title_idx_german on films (title collate "de_DE");

create index title_idx_nulls_low on films (title nulls first);

create index title_idx_nulls_high on films (title);

create unique index title_idx on films (title) with (fillfactor=70);

create index gin_idx on documents_table using gin (locations) with (fastupdate='off');

create index code_idx on films (code) tablespace indexspace;

create index pointloc on points using gist (box(location, location));

create index concurrently sales_quantity_index on sales_table (quantity);

create index super_idx on super_table using btree (super_column desc);

create index opclass_index on schema.opclass_table (col varchar_pattern_ops);

create index opclass_index_with_parameters on schema.opclass_table (col varchar_pattern_ops(p1='3', p2='4'));

create unique index tests_success_constraint on tests (subject, target) where success;

create index nulls_distinct_index on documents_table using gin (locations) with (fastupdate='off');

create index nulls_not_distinct_index on documents_table using gin (locations) nulls not distinct with (fastupdate='off');

create index code_idx on films (code) tablespace indexspace;

create index concurrently if not exists user_my_column_idx on my_schema.user (my_column);

create index table_a_idx_a on schema_a.table_a using bm25 (col_a, (col_a::pdb.unicode_words('alias=col_a_alias')));

create index idx_expr on t using btree ((col::my_type(10)));
