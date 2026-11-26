-- Not missing space before ON
select * from "my_table2"
inner join "my_database"."my_schema"."my_table"on ("my_table2".foo = "my_table".foo)
