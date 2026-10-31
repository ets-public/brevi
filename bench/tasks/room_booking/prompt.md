Book meeting rooms. Each stdin line is a command (ignore blank lines):

- `ROOM name capacity`: add a room.
- `BOOK id date start end people`: date is `YYYY-MM-DD`, start and end are `HH:MM` (end after start). Pick the room with the smallest capacity that is at least `people` and has no booking on that date overlapping the time (a booking ending exactly when another starts does not overlap). Ties: room name, alphabetically. Print `id -> room`, or `id rejected` if no room fits. Booking ids are unique.
- `CANCEL id`: print `id cancelled`, or `id not found` if there is no such active booking.
- `LIST date`: print `date Weekday` (the English weekday name, such as `Monday`), then one line per booking on that date: `  HH:MM-HH:MM room id`, sorted by start time, then room name. If there are none, print `  no bookings` after the header.
