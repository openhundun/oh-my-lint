select distinct
    field_1 from table_1
except (
    select distinct field_1
    from table_2
);

select field_1
from table_1
intersect
select field_1
    from table_2;

select field_1
from table_1
except
all (
    select field_1
    from table_2
);

select field_1
from table_1
union
select field_1
    from table_2;
