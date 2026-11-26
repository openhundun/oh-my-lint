create policy account_managers on accounts
    to current_user;

create policy account_managers on sch.accounts
    as permissive
    for all
    to managers;

create policy account_managers on accounts
    to public, session_user;

create policy account_managers on accounts
    with check (
        not accounts_is_excluded_full_name(full_name)
    );

create policy emp_rls_policy on employee for all to public using (ename = current_setting('rls.ename'));

create policy account_managers on accounts
    with check (
        col > 10
    );


create policy account_managers on accounts
    using (username = current_user);
