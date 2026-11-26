create domain us_postal_code as text
check (
   value ~ '^\d{5}$'
or value ~ '^\d{5}-\d{4}$'
);

create domain oname as text;

create domain mystr as text
constraint not_empty check (length(value) > 0)
constraint too_big check (length(value) <= 50000);

-- COLLATE and DEFAULT can come after the constraints, as pg_dump writes them.
CREATE DOMAIN public.posint AS integer NOT NULL DEFAULT 1
CONSTRAINT positive CHECK ((value > 0));

CREATE DOMAIN rating AS integer CHECK (value BETWEEN 1 AND 5) DEFAULT 3;

CREATE DOMAIN code AS text NOT NULL COLLATE "C" DEFAULT 'none';
