-- inner join
select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
inner join department
    on employee.deptno = department.deptno;

-- left join
select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
left join department
    on employee.deptno = department.deptno;

select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
left join department
    on employee.deptno = department.deptno;

-- right join
select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
right join department
    on employee.deptno = department.deptno;

select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
right join department
    on employee.deptno = department.deptno;

-- full join
select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
full join department
    on employee.deptno = department.deptno;

select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee
full join department
    on employee.deptno = department.deptno;

-- cross join
select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee cross join department;

select
    employee.id,
    employee.name,
    employee.deptno,
    department.deptname
from employee, department;
