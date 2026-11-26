DROP OPERATOR ^^ (text, text);

DROP OPERATOR IF EXISTS bob.!== (box, box);

drop operator if exists ~~~ (character varying, text[]), !! (none, integer) cascade;

drop operator public.<<-> (double precision, double precision) restrict;

DROP OPERATOR IF EXISTS === (box, box);
