alter policy account_managers on accounts
    rename to account_users;

alter policy account_managers on accounts
    to current_user;

alter policy account_managers on accounts
    to public, session_user;

alter policy account_managers on accounts
    with check (
        not accounts_is_excluded_full_name(full_name)
    );

alter policy account_managers on accounts
    with check (
        col > 10
    );

alter policy account_managers on accounts
    using (username = current_user);

alter policy sales_rep_is_self on invoices
  with check (sales_rep = current_user and current_user in (
    select user_id from allowed_users
  ));

alter policy test_policy on test_table
  to public, session_user
  using (username = current_user)
  with check (col > 10);
