select 'a fat cat sat on a mat and ate a fat rat'::tsvector @@ 'cat & rat'::tsquery;

select 'fat & cow'::tsquery @@ 'a fat cat sat on a mat and ate a fat rat'::tsvector;

select to_tsvector('fat cats ate fat rats') @@ to_tsquery('fat & rat');

select 'fat cats ate fat rats'::tsvector @@ to_tsquery('fat & rat');

select 'fat cats ate fat rats'::tsvector @@ to_tsquery('fat & rat');

select to_tsvector('error is not fatal') @@ to_tsquery('fatal <-> error');

select phraseto_tsquery('cats ate rats');

select phraseto_tsquery('the cats ate the rats');

select 'a:1 b:2'::tsvector || 'c:1 d:2 b:3'::tsvector;

select 'fat | rat'::tsquery && 'cat'::tsquery;

select 'fat | rat'::tsquery || 'cat'::tsquery;

select to_tsquery('fat') <-> to_tsquery('rat');

select 'cat'::tsquery @> 'cat & rat'::tsquery;

select 'cat'::tsquery <@ 'cat & rat'::tsquery;

select 'cat'::tsquery <@ '!cat & rat'::tsquery;

select !! 'cat'::tsquery;
