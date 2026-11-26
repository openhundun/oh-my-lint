listen virtual;
notify virtual;
unlisten virtual;

listen "virtual listener";
notify "virtual listener";
unlisten "virtual listener";

listen listener_a;
listen listener_b;
notify listener_a, 'payload_a';
notify listener_b, 'payload_b';
unlisten *;
