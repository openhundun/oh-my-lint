select u.user_id, u.user_email, p.product_id
from user_tb u
inner join product_tb p on u.user_id = p.user_id
  and position('@domain' in u.user_email) = 0;


select position('#' in tbl.col_a -> 'b') as hash_pos from tbl;
