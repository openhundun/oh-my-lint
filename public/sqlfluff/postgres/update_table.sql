update films set kind = 'Dramatic' where kind = 'Drama';

update weather set temp_lo = temp_lo + 1, temp_hi = temp_lo + 15, prcp = default
  where city = 'San Francisco' and date = '2003-07-03';

update weather set temp_lo = temp_lo + 1, temp_hi = temp_lo + 15, prcp = default
  where city = 'San Francisco' and date = '2003-07-03'
  returning temp_lo, temp_hi, prcp;

update weather set (temp_lo, temp_hi, prcp) = (temp_lo + 1, temp_lo + 15, default)
  where city = 'San Francisco' and date = '2003-07-03';

update employees set sales_count = sales_count + 1 from accounts
  where accounts.name = 'Acme Corporation'
  and employees.id = accounts.sales_person;

update employees set sales_count = sales_count + 1 where id =
  (select sales_person from accounts where name = 'Acme Corporation');

update accounts set (contact_first_name, contact_last_name) =
    (select first_name, last_name from salesmen
     where salesmen.id = accounts.sales_id);

update accounts set contact_first_name = first_name,
                    contact_last_name = last_name
  from salesmen where salesmen.id = accounts.sales_id;

update summary s set (sum_x, sum_y, avg_x, avg_y) =
    (select sum(x), sum(y), avg(x), avg(y) from data d
     where d.group_id = s.group_id);

UPDATE films SET kind = 'Dramatic' WHERE CURRENT OF c_films;

update my_table set my_column = "SQLFluff rules!" returning my_column;

update employees
set deleted_at = now()
where uuid = $1
returning short_name as employee_name;
