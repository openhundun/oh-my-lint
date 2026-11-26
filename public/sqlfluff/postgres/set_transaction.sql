set transaction isolation level serializable;

set transaction isolation level repeatable read;

set transaction isolation level read committed, read write;

SET TRANSACTION READ ONLY, DEFERRABLE;

-- PostgreSQL also accepts the modes without commas
SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED READ ONLY NOT DEFERRABLE;

set transaction snapshot '00000003-0000001B-1';

set local transaction read only;

set session characteristics as transaction isolation level serializable;

SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY, NOT DEFERRABLE;

SET LOCAL SESSION CHARACTERISTICS AS TRANSACTION READ WRITE;
