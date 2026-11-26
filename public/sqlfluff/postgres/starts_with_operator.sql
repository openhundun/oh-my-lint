select 'alphabet' ^@ 'alph';

select 'alphabet'^@'alph';

select first_name
from customers
where first_name ^@ 'Jo' and not last_name ^@ 'Sm';

select 'alphabet' operator(pg_catalog.^@) 'alph';
