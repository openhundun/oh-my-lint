-- PostgreSQL 17 added a RETURNING clause to MERGE.
-- https://www.postgresql.org/docs/17/sql-merge.html

merge into customer_account ca
using recent_transactions t
on t.customer_id = ca.customer_id
when matched then
    update set balance = balance + transaction_value
returning ca.customer_id, ca.balance;

merge into wines w
using wine_stock_changes s
on s.winename = w.winename
when not matched and s.stock_delta > 0 then
    insert values (s.winename, s.stock_delta)
when matched and w.stock + s.stock_delta > 0 then
    update set stock = w.stock + s.stock_delta
when matched then
    delete
returning merge_action(), w.*;

merge into t
using s
on t.id = s.id
when matched then
    update set a = s.a
returning *;

merge into t
using s
on t.id = s.id
when matched then
    update set a = s.a
returning merge_action() as action, t.id as ident, t.a * 2;
