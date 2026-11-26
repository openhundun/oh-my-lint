alter role davide with password 'hu8jmn3';
alter role davide with password null;
alter role chris valid until 'May 4 12:00:00 2015 +1';
alter role fred valid until 'infinity';
alter role worker_bee set maintenance_work_mem = '100000';
alter role fred in database devel set client_min_messages to default;
alter role miriam createrole createdb;
alter user davide with password 'hu8jmn3';
alter user davide with password null;
alter user chris valid until 'May 4 12:00:00 2015 +1';
alter user fred valid until 'infinity';
alter user worker_bee set maintenance_work_mem = '100000';
alter user fred in database devel set client_min_messages to default;
alter user miriam createrole createdb;

-- more SET tests:
alter role fred set testing from current;
alter role fred in database devel set testing from current;
alter role fred in database devel set testing to 1234;
alter role fred in database devel set testing = 1234;
alter role fred in database devel set testing to default;
alter role fred in database devel set testing = default;
alter role fred in database devel set testing = true;
alter role fred in database devel set testing = false;
alter role fred in database devel set testing = 'string value';
ALTER ROLE fred IN DATABASE devel SET testing = on, off, auto;

alter role fred reset all;
alter role fred reset testing;
alter role fred in database devel reset all;
alter role fred in database devel reset testing;

-- CURRENT_ROLE/CURRENT_USER/SESSION_USER and ALL
alter user current_role with password null;
alter user current_user with password null;
alter user session_user with password null;

alter role current_role in database devel set testing from current;
alter role current_role in database devel set testing to 1234;
alter role current_role in database devel set testing = 1234;
alter role current_role in database devel set testing to default;

alter role all reset all;
alter role all reset testing;
alter user all in database devel reset all;
alter role all in database devel reset testing;
