select * from items order by embedding <-> '[3,1,2]' limit 5;

select * from items order by embedding <=> '[3,1,2]' limit 5;

SELECT * FROM items ORDER BY embedding <+> '[3,1,2]' LIMIT 5;

select * from items order by embedding <#> '[3,1,2]' limit 5;
