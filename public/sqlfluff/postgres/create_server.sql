create server test foreign data wrapper postgres_fdw;

create server if not exists test foreign data wrapper oracle_fdw;

create server test type 'test' version '1.0' foreign data wrapper postgres_fdw;

create server test foreign data wrapper postgres_fdw options (host 'foo', dbname 'foodb', port '5432');

create server test foreign data wrapper postgres_fdw options ("host" 'foo', port '5432');
