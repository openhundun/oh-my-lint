insert into foo (bar) values (current_timestamp);

insert into foo (bar, baz) values (1, 2), (3, 4);

insert into foo (bar, baz) values (1 + 1, 2), (3, 4);

insert into foo (bar) values (default);

insert into distributors as d (did, dname) values (8, 'Anvil Distribution');

insert into test (id, col1) overriding system value values (1, 'val');

insert into test (id, col1) overriding user value values (1, 'val');

INSERT INTO foo (bar) DEFAULT VALUES;

insert into films select * from tmp_films where date_prod < '2004-05-07';

insert into foo (bar) values (current_timestamp)
returning *;

insert into foo (bar) values (current_timestamp)
returning bar;

insert into foo (bar) values (current_timestamp)
returning bar as some_alias;

insert into foo (bar, baz) values (1, 2)
returning bar, baz;

insert into foo (bar, baz) values (1, 2)
returning bar as alias1, baz as alias2;

insert into foo (bar, baz) values (1, 'var')
on conflict (bar) do update set baz = excluded.baz;

insert into foo (bar, baz) values (1, 'var')
on conflict (bar) do nothing;

insert into foo as f (bar, baz) values (1, 'var')
on conflict (bar) do update set baz = excluded.baz || ' (formerly ' || f.baz || ')'
where f.zipcode != '21201';

insert into foo (bar, baz) values (1, 'var')
on conflict on constraint foo_pkey do nothing;

insert into foo (bar, baz) values (1, 'var')
on conflict (bar) where is_active do nothing;

insert into foo (bar, baz) values (1, 'var')
on conflict (bar) do update set (baz) = (select baz from foobar where bar = 1);

insert into megatable (megacolumn)
select * from (
    values ( 'megavalue' )
) tmp(megacolumn)
where not exists (
select from megatable mt
    where mt.megacolumn = tmp.megacolumn
)
on conflict do nothing;

insert into abc (foo, bar)
select foo, bar from baz
returning quux
;

INSERT INTO tbl_a (
   val1
 , val2
)
SELECT val1
     , val2
FROM tbl_2
ON CONFLICT (
    val1
  , COALESCE(val2, '')
)
DO NOTHING;

insert into prompt_variants (
    test,
    test2
)
select
    test,
    test2
returning
  as test,
  test2;

INSERT INTO
  baz (state, state_changed_at, instance_id)
SELECT
  1, 2, 3
ON CONFLICT (instance_id) DO UPDATE
SET
  state = foo,
  state_changed_at = bar;
