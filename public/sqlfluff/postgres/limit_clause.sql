select col_a
from test_table
limit 2 * 5 * 10 offset(5 + 10);


select col_a
from test_table
limit (10 / 10) offset 10 - 5;


select col_a
from test_table
limit 100;


select col_a
from test_table
limit all
