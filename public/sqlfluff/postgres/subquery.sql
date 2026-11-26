insert into target_table (target_column)
select table1.column1
from table1
inner join (
    select table2.join_column
    from table2
) temp3
on table1.join_column = temp3.join_column;
