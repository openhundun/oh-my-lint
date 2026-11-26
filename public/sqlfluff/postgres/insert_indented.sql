-- Indented INSERT target with nested VALUES rows.
insert into
    assets (
        id,
        filename,
        mime_type
    )
values
    (1, 'file.txt', 'text/plain'),
    (2, 'image.png', 'image/png');

insert into foo (bar, baz) values (1, 2), (3, 4);
