create trigger foo after insert on bar execute function proc(args);

create trigger foo before insert on bar execute function proc(args);

create trigger foo after update of bar, baz on bar execute function proc(args);

create trigger foo instead of delete on bar from baz deferrable initially deferred for each statement execute function proc(args);

create trigger foo instead of delete on bar from baz deferrable initially deferred for each statement execute function schema.proc(args);

create trigger foo before insert on bar when (a = b) execute function proc(args);

create or replace constraint trigger foo before insert on bar execute function proc(args);

create trigger foo before insert on bar referencing old table as old_table new table as new_table execute procedure proc(args);

create trigger check_update
    before insert or update on accounts
    for each row
    execute function check_account_update();

create or replace trigger check_update
    before update of balance, transactions on accounts
    for each row
    execute function check_account_update();

create or replace trigger check_update
    before update of balance, transactions or truncate on accounts
    for each row
    execute function check_account_update();

create or replace trigger check_update
    before update of balance on accounts
    for each row
    execute function check_account_update();

create trigger check_update
    before update on accounts
    for each row
    when (old.balance is distinct from new.balance)
    execute function check_account_update();

--CREATE TRIGGER log_update
--    AFTER UPDATE ON accounts
--    FOR EACH ROW
--    WHEN (OLD.* IS DISTINCT FROM NEW.*)
--    EXECUTE FUNCTION log_account_update();

create trigger view_insert
    instead of insert on my_view
    for each row
    execute function view_insert_row();

create trigger transfer_insert
    after insert on transfer
    referencing new table as inserted
    for each statement
    execute function check_transfer_balances_to_zero();

create trigger paired_items_update
    after update on paired_items
    referencing new table as newtab old table as oldtab
    for each row
    execute function check_matching_pairs();

create trigger log_update
    after update on accounts
    for each row
    when (old.* is distinct from new.*)
    execute function log_account_update();
