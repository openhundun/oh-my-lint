create collation numeric(provider=icu, locale='en@colNumeric=yes');

create collation french(locale='fr_FR.utf8');

create collation german_phonebook(provider=icu, locale='de-u-co-phonebk');

create collation german from "de_DE";

create collation c(provider=icu, locale='und', deterministic=off);
