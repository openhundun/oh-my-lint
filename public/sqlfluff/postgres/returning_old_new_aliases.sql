-- PostgreSQL 18 allows RETURNING to rename the OLD and NEW row references.
-- https://www.postgresql.org/docs/18/sql-update.html

update products set price = price * 1.10
where price <= 99.99
returning with (old as o, new as n) name, o.price as old_price, n.price as new_price;

INSERT INTO products (product_no, name, price) VALUES (1, 'Cheese', 9.99)
ON CONFLICT (product_no) DO UPDATE SET price = excluded.price
RETURNING WITH (NEW AS n) n.*;

DELETE FROM products
WHERE price = 0
RETURNING WITH (OLD AS removed) removed.product_no;

MERGE INTO products p
USING new_prices np
ON p.product_no = np.product_no
WHEN MATCHED THEN
    UPDATE SET price = np.price
RETURNING WITH (NEW AS n, OLD AS o) merge_action(), o.price, n.price;
