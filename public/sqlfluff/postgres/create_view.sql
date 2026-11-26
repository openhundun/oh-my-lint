create view vista as select 'Hello World';

create or replace view vista as select 'Hello World';

create view vista as select text 'Hello World' as hello;

create temp view vista as select text 'Hello World' as hello;

create temporary view  vista as select text 'Hello World' as hello;

create view comedies as
    select *
    from films
    where kind = 'Comedy';

CREATE VIEW pg_comedies AS
    VALUES (1, 'one'), (2, 'two'), (3, 'three')
    WITH LOCAL CHECK OPTION;

CREATE VIEW pg_comedies AS
    SELECT *
    FROM comedies
    WHERE classification = 'PG'
    WITH CASCADED CHECK OPTION;
create view foo with (security_invoker) as select 1;
create view foo with (security_barrier) as select 1;

create view foo with (security_invoker=boolean) as select 1;
create view foo with (security_barrier=boolean) as select 1;

create view foo with (check_option=local) as select * from other_view;
create view foo with (check_option=cascaded) as select * from other_view;

create view foo with (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc)
    as select 1;

create view foo as select * from OTHER_VIEW with local check option;
create view foo as select * from OTHER_VIEW with cascaded check option;

CREATE OR REPLACE RECURSIVE VIEW "grouping_node" (
  "node_id",
  "ancestors",
  "category_id",
  "path",
  "path_nodes"
) AS

SELECT "group_id" AS "node_id",
       ARRAY[]::INTEGER[] AS "ancestors",
       "category_id",
       ARRAY["name"]::text[] AS "path",
       ARRAY["group_id"]::INTEGER[] AS "path_nodes"
  FROM "grouping_managementgroup"
 WHERE "parent_id" IS NULL

 UNION ALL

SELECT "group_id",
       "ancestors" || "parent_id",
       "grouping_node"."category_id",
       "path" || "name"::text,
       "path_nodes" || "group_id"
FROM "grouping_managementgroup", "grouping_node"
WHERE "parent_id" = "node_id";

-- use of collation as non-reserved keyword
create view foo as select col1 as collation from other_view;
