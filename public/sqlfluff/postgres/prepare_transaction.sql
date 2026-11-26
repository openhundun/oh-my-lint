begin;
insert into t values (3);
PREPARE TRANSACTION 'foobar';
COMMIT PREPARED 'foobar';

begin;
insert into t values (4);
PREPARE TRANSACTION 'foobar2';
ROLLBACK PREPARED 'foobar2';
