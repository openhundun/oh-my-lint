create rule protect_orders as on delete to orders do instead nothing;

create or replace rule notify_orders as on update to orders do also notify orders_changed;

create rule log_order_insert as on insert to orders
    do insert into order_log values (new.id, 'inserted');

create rule archive_order as on delete to sales.orders
    where old.status = 'closed'
    do also (
        insert into order_log values (old.id, 'archived');
        update order_totals set n = n - 1 where customer_id = old.customer_id
    );

create rule notify_order_insert as on insert to sales.orders
    do also (; notify orders_changed;; notify order_inserted, 'new';);

create rule ignore_insert as on insert to order_totals do instead ();

create rule select_on_update as on update to order_totals do also ((select 1));

create rule log_order_delete as on delete to order_totals
    do also with note as (select 'deleted' as txt)
    insert into order_log select old.customer_id, txt from note;

create rule soft_delete as on delete to customers
    do instead update customers set deleted = true where id = old.id returning *;

CREATE OR REPLACE RULE "_RETURN" AS ON SELECT TO order_view DO INSTEAD SELECT 2 AS a;
