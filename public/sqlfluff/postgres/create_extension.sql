CREATE EXTENSION amazing_extension
    with schema schema1
    VERSION '2.0.1.2'
    FROM '1.0';

CREATE EXTENSION IF NOT EXISTS amazing_extension
    with schema schema1
    VERSION '1.2.3a4'
    FROM '1.0';

CREATE EXTENSION amazing_extension
    with schema schema1
    VERSION version_named
    FROM from_named;

create extension if not exists aws_lambda cascade;

drop extension amazing_extension;

drop extension if exists amazing_extension;
