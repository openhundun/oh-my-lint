create user mapping for bob server foo options (user 'bob', password 'secret');

create user mapping if not exists for public server foo;

create user mapping if not exists for current_user server foo options (user 'bob', password 'secret', option 'value');

-- pg_dump quotes the option names that PostgreSQL reserves
create user mapping for bob server foo options (
    password 'secret',
    "user" 'bob'
);

create user mapping for user server foo;
