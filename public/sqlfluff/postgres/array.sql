select array[1, 2] || array[3, 4];

select
    array[['meeting',
    'lunch'],
    ['training',
    'presentation']];

create table sal_emp (name text, pay_by_quarter int[], schedule text[][]);

create table tictactoe (squares int[3][3]);

select *
from sal_emp
where pay_by_quarter[1] = 10000 or
                            pay_by_quarter[2] = 10000 or
                            pay_by_quarter[3] = 10000 or
                            pay_by_quarter[4] = 10000;

insert into sal_emp
    values ('Bill',
    array[10000, 10000, 10000, 10000],
    array[['meeting', 'lunch'], ['training', 'presentation']]);

insert into sal_emp
    values ('Carol',
    array[20000, 25000, 25000, 25000],
    array[['breakfast', 'consulting'], ['meeting', 'lunch']]);

select name from sal_emp where pay_by_quarter[1] != pay_by_quarter[2];

select schedule[1:2][1:1] from sal_emp where name = 'Bill';

UPDATE sal_emp SET pay_by_quarter[4] = 15000
    WHERE name = 'Bill';

UPDATE sal_emp SET pay_by_quarter[1:2] = '{27000,27000}'
    WHERE name = 'Carol';

select array_dims(array[1, 2] || array[3, 4, 5]);

select array_dims(array[1, 2] || array[[3, 4], [5, 6]]);

select array[1, 2] || '{3, 4}';

select array_position(array['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'], 'mon');

select f1[1][-2][3] as e1, f1[1][-1][5] as e2
 from (select '[1:1][-2:-1][3:5]={{{1,2,3},{4,5,6}}}'::int[] as f1) ss;

select '{Hello,World}'::_text as text_array;

select array['A', 'B', 'C']::_text;

select sum(case
        when direction = 'forward' then unit
        else 0
        end
    ) * (max(array[id, vertical]))[2]
from direction_with_vertical_change;

-- More advanced cases with expressions and missing slice start/end when accessing

select
    a[:],
    b[:1],
    c[2:],
    d[2:3];

select a[1 + 2:3 + 4], b[5 + 6];
