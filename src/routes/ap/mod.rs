pub mod inbox;
pub mod outbox;


// @todo -- inboxes?
// The inbox stream contains all activities received by the actor. The server SHOULD filter content according to the requester's permission. In general, the owner of an inbox is likely to be able to access all of their inbox contents. Depending on access control, some other content may be public, whereas other content may require authentication for non-owner users, if they can access the inbox at all. 