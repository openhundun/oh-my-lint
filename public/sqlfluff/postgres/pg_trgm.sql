-- PostgreSQL pg_trgm similarity operators
-- https://www.postgresql.org/docs/current/pgtrgm.html

-- text % text → boolean (similarity)
select 'abc' % 'abd';

-- text <% text → boolean (word_similarity)
select 'word' <% 'some word here';

-- text %> text → boolean (word_similarity reverse)
select 'some word here' %> 'word';

-- text <<% text → boolean (strict_word_similarity)
select 'text' <<% 'some text example';

-- text %>> text → boolean (strict_word_similarity reverse)
select 'some text example' %>> 'text';

-- text <-> text → real (similarity distance)
select 'str1' <-> 'str2';

-- text <<-> text → real (word_similarity distance)
select 'item' <<-> 'some item value';

-- text <->> text → real (word_similarity distance reverse)
select 'some item value' <->> 'item';

-- text <<<-> text → real (strict_word_similarity distance)
select 'name' <<<-> 'some name field';

-- text <->>> text → real (strict_word_similarity distance reverse)
select 'some name field' <->>> 'name';
