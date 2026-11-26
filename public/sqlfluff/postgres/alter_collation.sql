alter collation german refresh version;

alter collation german rename to "de-DE";

alter collation "de-DE" owner to joe;

alter collation public."de-DE" owner to current_user;

alter collation "de-DE" set schema utils;

alter collation utils."de-DE" owner to session_user;

alter collation utils."de-DE" owner to current_role;
