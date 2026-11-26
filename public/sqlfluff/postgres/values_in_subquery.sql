with
t(col_1, col_2) as (
    values
    ('08RIX0', 0.435::numeric(4, 3))
)

select *
from t;

select *
from (
    values (1)
) t(c1);

select *
from (
    values (1, 2), (3, 4)
) t(c1, c2);
