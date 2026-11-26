select * from tbl1

except

select * from tbl2
union all
(
    select * from tbl2
    except
    select * from tbl1
    order by column_1
)
order by column_2;
