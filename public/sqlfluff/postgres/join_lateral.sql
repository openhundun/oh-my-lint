-- Postgres should work with standard joins
select tbl1.id from tbl1 join tbl2 on tbl1.id = tbl2.id;

-- ... but also with lateral joins
select tbl1.id from tbl1 join lateral(select * from tbl2) foo on tbl1.id = foo.id;

-- ... and mixed ones as well!
select tbl1.id from tbl1
full join lateral(select * from tbl2) tbl2 on tbl1.id = tbl2.id
cross join tbl3
left join lateral(select * from tbl4) tbl4 on tbl1.id = tbl4.id;

-- lateral with comma cross join syntax
select x.num, d.id from tbl1 d, lateral(values (0), (1)) x(num);

-- lateral with function
select m.name as mname, pname
from manufacturers m, lateral get_product_names(m.id) pname;

select m.name as mname, pname
from manufacturers m
left join lateral get_product_names(m.id) pname on true;

select x.num
from lateral(values (0), (1)) x(num);
