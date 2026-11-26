prepare select_statement as
select * from table1;

prepare insert_statement as
insert into table1 (col1, col2) values (1, 'foo');

prepare update_statement as
update table1 set col2 = 'bar' where col1 = 1;

prepare delete_statement as
delete from table1 where col1 = 1;

prepare values_statement as
values (1, 'foo');

prepare merge_statement as
merge into table1 using table2 on (table1.col1 = table2.col1)
when matched then
update set col1 = table2.col1, col2 = table2.col2
when not matched then
insert(col1, col2) values (table2.col1, table2.col2);

prepare parametrized_statement_1(int) as
values ($1);

prepare parametrized_statement_2(int, character(3)) as
values ($1, $2);
