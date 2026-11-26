ALTER OPERATOR ^^ (text, text) OWNER TO joe;

ALTER OPERATOR public.=== (box, box) OWNER TO CURRENT_USER;

alter operator ~~~ (character varying, text[]) owner to session_user;

alter operator !! (none, integer) owner to current_role;

ALTER OPERATOR !== (box, box) SET SCHEMA bob;

ALTER OPERATOR bob.!== (box, box) SET (RESTRICT = neqsel, JOIN = neqjoinsel);

ALTER OPERATOR bob.!== (box, box) SET (RESTRICT = NONE, JOIN = NONE);

ALTER OPERATOR === (box, box) SET (COMMUTATOR = ===, NEGATOR = OPERATOR(bob.!==));

ALTER OPERATOR === (box, box) SET (HASHES, MERGES);
