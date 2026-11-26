create foreign table foreign_table (
    simple_column integer,
    column_with_options char(5) options (a 'foo', b 'bar'),
    column_with_collate text collate "de_DE",
    column_with_not_null_constraint date not null,
    column_with_null_constraint varchar(50) null,
    column_with_check_constraint float check (column_with_check_constraint > 0.0),
    column_with_default_constraint timestamp default current_timestamp,
    column_with_generated_constraint bigint generated always as (simple_column * 2) stored,
    column_with_more_than_one_constraint int not null check (column_with_more_than_one_constraint > 0),
    column_with_options_and_collate char(5) options (a 'foo', b 'bar') collate "es_ES",
    column_with_options_and_constraint char(5) options (a 'foo', b 'bar') not null,
    column_with_collate_and_constraint char(5) collate "de_DE" not null,
    column_with_options_collate_and_constraint char(5) options (a 'foo', b 'bar') collate "de_DE" not null,
    check (simple_column > 0),
    check (simple_column < 10) no inherit,
    constraint named_table_constraint check (column_with_options != ''),
    constraint named_table_constraint_no_inherit check (column_with_collate != '') no inherit
)
server a_server;

create foreign table if not exists foreign_table_that_might_already_exist(
    simple_column integer
)
server a_server;

create foreign table foreign_table_that_inherits (
    simple_column integer
)
inherits ( another_table )
server a_server;

create foreign table if not exists foreign_table_that_inherits_that_might_already_exist(
    simple_column integer
)
inherits ( another_table )
server a_server;

create foreign table foreign_table_with_options (
    simple_column integer
)
server a_server
options (c 'baz');

create foreign table if not exists foreign_table_with_options_that_might_already_exist(
    simple_column integer
)
server a_server
options (c 'baz');

create foreign table foreign_table_that_inherits_and_has_options (
    simple_column integer
)
inherits ( another_table )
server a_server
options (c 'baz');

create foreign table if not exists foreign_table_that_inherits_and_has_options_that_might_already_exist(
    simple_column integer
)
inherits ( another_table )
server a_server
options (c 'baz');

create foreign table foreign_table_partition_in
    partition of another_table for values in ('2016-07-01', '2016-08-01')
    server a_server;

create foreign table foreign_table_partition_from_min_to_max
    partition of another_table for values from ('2016-07-01') to ('2016-08-01')
    server a_server;

create foreign table foreign_table_partition_with
    partition of another_table for values with ( modulus 2, remainder 0)
    server a_server;

create foreign table if not exists foreign_table_partition_in_that_might_already_exist
    partition of another_table for values in ('2016-07-01', '2016-08-01')
    server a_server;

create foreign table if not exists foreign_table_partition_from_min_to_max_that_might_already_exist
    partition of another_table for values from ('2016-07-01') to ('2016-08-01')
    server a_server;

create foreign table if not exists foreign_table_partition_with_that_might_already_exist
    partition of another_table for values with ( modulus 2, remainder 0)
    server a_server;

create foreign table foreign_table_partition_in_with_options
    partition of another_table for values in ('2016-07-01', '2016-08-01')
    server a_server
    options (foo 'bar');

create foreign table foreign_table_partition_from_min_to_max_with_options
    partition of another_table for values from ('2016-07-01') to ('2016-08-01')
    server a_server
    options (foo 'bar');

create foreign table foreign_table_partition_with_with_options
    partition of another_table for values with ( modulus 2, remainder 0)
    server a_server
    options (foo 'bar');

create foreign table foreign_table_partition_in_with_columns
    partition of another_table(
        simple_column,
        column_with_options with options,
        column_with_not_null_constraint not null,
        column_with_null_constraint null,
        column_with_check_constraint check (column_with_check_constraint > 0.0),
        column_with_default_constraint default current_timestamp,
        column_with_generated_constraint generated always as (simple_column * 2) stored,
        column_with_more_than_one_constraint not null check (column_with_more_than_one_constraint > 0),
        column_with_options_and_not_null_constraint with options not null,
        check (simple_column > 0),
        check (simple_column < 10) no inherit,
        constraint named_table_constraint check (column_with_options != ''),
        constraint named_table_constraint_no_inherit check (column_with_options_and_not_null_constraint != '')  no inherit
    )
    for values in ('2016-07-01', '2016-08-01')
    server a_server;

create foreign table foreign_table_partition_with_from_min_to_max_with_columns
    partition of another_table(
        simple_column
    )
    for values from ('2016-07-01') to ('2016-08-01')
    server a_server;

create foreign table foreign_table_partition_with_with_columns
    partition of another_table(
        simple_column
    )
    for values with ( modulus 2, remainder 0)
    server a_server;

-- `null` is a reserved word, which pg_dump writes quoted
create foreign table measurements (
    city text options ("force_not_null" 'true'),
    peak int
)
server files
options (filename '/data/measurements.csv', format 'csv', null '');

create foreign table measurements_dumped (
    city text,
    peak int
)
server files
options (filename '/data/measurements.csv', format 'csv', "null" '');
