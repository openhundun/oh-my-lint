ALTER TEXT SEARCH CONFIGURATION my_config ADD MAPPING FOR asciiword WITH english_stem;
ALTER TEXT SEARCH CONFIGURATION my_config ALTER MAPPING FOR word WITH french_stem;
ALTER TEXT SEARCH CONFIGURATION my_config ALTER MAPPING REPLACE english WITH french;
ALTER TEXT SEARCH CONFIGURATION my_config ALTER MAPPING FOR asciiword REPLACE english_stem WITH french_stem;
ALTER TEXT SEARCH CONFIGURATION my_config DROP MAPPING IF EXISTS FOR email;
alter text search configuration my_config rename to new_config;
alter text search configuration my_config owner to new_owner;
alter text search configuration my_config set schema new_schema;
