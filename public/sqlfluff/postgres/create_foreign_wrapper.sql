create foreign data wrapper stripe_wrapper
    handler stripe_fdw_handler
    validator stripe_fdw_validator;

create foreign data wrapper dummy;

create foreign data wrapper no_functions no handler no validator;

-- The clauses can come in any order, and the functions can be schema-qualified
create foreign data wrapper qualified
    validator ext.my_validator
    handler ext.my_handler;

create foreign data wrapper with_options
    handler postgres_fdw_handler
    options (debug 'true', retries '3');

create foreign data wrapper quoted_options options ("debug" 'true');
