alter text search dictionary my_english(stopwords=english);

alter text search dictionary simple_dict(accept);

alter text search dictionary simple_dict(accept=true, stopwords='english');

alter text search dictionary my_english owner to joe;

alter text search dictionary public.my_english owner to current_user;

alter text search dictionary my_english owner to current_role;

alter text search dictionary my_english owner to session_user;

alter text search dictionary my_english rename to my_stem;

alter text search dictionary my_stem set schema utils;

alter text search parser my_parser rename to my_prs;

alter text search parser my_prs set schema utils;

alter text search template my_template rename to my_tmpl;

alter text search template my_tmpl set schema utils;
