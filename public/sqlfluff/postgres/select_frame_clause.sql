select
    venuestate,
    venueseats,
    venuename,
    first_value(venuename ignore nulls)
    over(partition by venuestate
                      order by venueseats desc
        rows between unbounded preceding and unbounded following) as col_name
from table_name;

SELECT rank () OVER (ORDER BY my_column RANGE BETWEEN 12 FOLLOWING AND CURRENT ROW EXCLUDE NO OTHERS);

SELECT rank () OVER (ORDER BY my_column GROUPS UNBOUNDED PRECEDING EXCLUDE GROUP);

SELECT rank () OVER (ORDER BY my_column RANGE BETWEEN
        INTERVAL '1 YEAR - 1 DAYS' PRECEDING AND
        INTERVAL '15 DAYS' PRECEDING);

-- Frame clauses without ORDER BY
select sum(x) over(rows between unbounded preceding and current row) from t;

select sum(x) over(rows unbounded preceding) from t;

select sum(x) over(range between unbounded preceding and unbounded following) from t;

select sum(x) over(partition by y rows between 1 preceding and 1 following) from t;
