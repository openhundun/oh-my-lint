-- postgres_pattern_match_expressions.sql
/* examples of pattern match expressions
( https://www.postgresql.org/docs/14/functions-matching.html ) that are
supported in postgres. */

-- LIKE/ILIKE expressions supported
select *
from animals
where family like '%ursidae%';

select *
from animals
where family not like '%ursidae%';

select *
from animals
where genus ilike '%ursus%';

select *
from animals
where genus not ilike '%ursus%';

select *
from animals
where family like '%ursidae%' escape '\\';

select *
from animals
where genus not ilike '%ursus%' escape '\\';

select coalesce(family like '%ursidae%' escape '\\', false) as is_bear
from animals;

-- SIMILAR TO expressions supported
select *
from animals
where family similar to '%ursidae%';

select *
from animals
where family not similar to '%ursidae%';

select *
from animals
where genus similar to '%ursus%';

select *
from animals
where genus not similar to '%ursus%';

select *
from animals
where family similar to '%ursidae%' escape '\\';

select *
from animals
where genus not similar to '%ursus%' escape '\\';

select coalesce(family similar to '%ursidae%' escape '\\', false) as is_bear
from animals;

-- From https://github.com/sqlfluff/sqlfluff/issues/2722
with cleaned_bear_financial_branch as (
    select
        branch_id,
        to_number(case when honey_numerical_code similar to '[0-9]{0,7}.?[0-9]{0,2}' then honey_numerical_code else null end, '24601') as honey_numerical_code
    from bear_financial_branch
)

select branch_id
from cleaned_bear_financial_branch
limit 10;
