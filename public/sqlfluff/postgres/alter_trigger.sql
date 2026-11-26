alter trigger foo on table_name rename to new_name;

alter trigger foo on table_name depends on extension extension_name;

alter trigger foo on table_name no depends on extension extension_name;

alter trigger emp_stamp on emp rename to emp_track_chgs;

alter trigger emp_stamp on emp depends on extension emplib;
