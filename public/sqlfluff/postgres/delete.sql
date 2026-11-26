delete from films;

delete from only films;

delete from films *;

delete from films f;

delete from films f;

delete from films
using producers
where producer_id = producers.id and producers.name = 'foo';

delete from films f
using producers p
where f.producer_id = p.id and p.name = 'foo';

delete from films f
using producers p, actors a
where 1 = 1
    and f.producer_id = p.id
    and p.name = 'foo'
    and f.actor_id = a.id
    and a.name = 'joe cool';

delete from films f
using producers p
where f.producer_id = p.id and p.name = 'foo';

delete from films f
using producers p, actors a
where 1 = 1
    and f.producer_id = p.id
    and p.name = 'foo'
    and f.actor_id = a.id
    and a.name = 'joe cool';

delete from tasks where current of c_tasks;

delete from films where kind != 'Musical';

delete from tasks where status = 'DONE' returning *;

delete from tasks where status = 'DONE' returning actor_id;

delete from tasks where status = 'DONE' returning actor_id as a_id;

delete from tasks where status = 'DONE' returning actor_id as a_id;

delete from tasks where status = 'DONE' returning actor_id, producer_id;

delete from tasks where status = 'DONE' returning actor_id as a_id, producer_id as p_id;

delete from tasks where status = 'DONE' returning actor_id as a_id, producer_id as p_id;

with test as (select foo from bar) delete from films;

with recursive t(n) as (
    values (1)
    union all
    select n + 1 from t where n < 100
)
delete from films;

delete from foo
using baz
left join bar on baz.bar_id = bar.id
where foo.id = bar.foo_id and baz.id = 'c';
