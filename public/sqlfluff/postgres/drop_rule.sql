drop rule keep_orders on orders;

drop rule if exists "Archive Order" on sales.orders;

drop rule notify_orders on orders cascade;

drop rule if exists log_order_insert on orders restrict;
