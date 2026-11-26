begin;
commit;

begin work isolation level serializable;
commit work;

BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY, DEFERRABLE;
commit transaction;

-- PostgreSQL also accepts transaction modes without commas
BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE NOT DEFERRABLE;
end;

start transaction isolation level read uncommitted;
end transaction and no chain;

start transaction read only;
commit and chain;
rollback and no chain;

begin;
abort;

begin;
abort work and chain;
rollback work;
