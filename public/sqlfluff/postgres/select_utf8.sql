-- Postgres should work with Unicode identifiers in various places

select größe, länge from measurements;

select width as größe, height as höhe from dimensions;

select 'Não' as reativação from table1;

select αλφα, βητα, γαμμα from greek_letters;

select москва, санкт_петербург from cities;

select field1 as 名前 from users;

select id, größe, description from products;

with
größen as (
    select * from base_table
)
select *
from größen;

create table größen (länge int, breite int);
