-- Test OPERATOR() syntax for qualified operator references
-- https://www.postgresql.org/docs/current/sql-syntax-lexical.html#SQL-SYNTAX-OPERATORS

-- Basic comparison operators with schema qualification
select *
from table1
where col1 operator(public. = ) 'value1';

select *
from table1
where col2 operator(pg_catalog. != ) 'value2';

select *
from table1
where col3 operator(myschema. > ) 100;

select *
from table1
where col4 operator(myschema. < ) 100;

select *
from table1
where col5 operator(public. >= ) 50;

select *
from table1
where col6 operator(public. <= ) 50;

-- Arithmetic operators
select col1 operator(public.+) col2 as sum_result
from table1;

select col1 operator(public.-) col2 as diff_result
from table1;

select col1 operator(public.*) col2 as mult_result
from table1;

select col1 operator(public. / ) col2 as div_result
from table1;

-- String operators
select str1 operator(public.||) str2 as concatenated
from table1;

-- OPERATOR in HAVING clause
select category, count(*)
from products
group by category
having count(*) operator(public. > ) 5;

-- OPERATOR in JOIN condition
select t1.id, t2.name
from table1 t1
inner join table2 t2 on t1.id operator(public. = ) t2.table1_id;

-- OPERATOR in CASE expression
select
    (case
        when value operator(public. > ) 100 then 'high'
        when value operator(public. <= ) 100 then 'low'
    end) as category
from measurements;

-- Multiple schema names
select *
from table1
where col1 operator(schema1. = ) val1
  and col2 operator(schema2. != ) val2;

-- Operators of more than one character, with a schema, a quoted schema or none
SELECT *
FROM table1
WHERE col1 OPERATOR (myschema.===) col2
  AND col2 OPERATOR ("My Schema".===) col3
  AND col3 OPERATOR (<<<) col4;
