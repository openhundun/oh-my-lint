create text search dictionary my_english(
    template=snowball,
    language=english,
    stopwords=english
);

create text search dictionary public.simple_dict(
    template=pg_catalog.simple,
    stopwords='english',
    accept=false
);

create text search parser my_parser(
    start=prsd_start,
    gettoken=prsd_nexttoken,
    end=prsd_end,
    lextypes=prsd_lextype,
    headline=prsd_headline
);

create text search template my_template(
    init=dsimple_init,
    lexize=dsimple_lexize
);

create text search template lexize_only(lexize=dsimple_lexize);
