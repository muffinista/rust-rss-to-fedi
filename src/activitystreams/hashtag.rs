use activitystreams::link::Link;

use activitystreams::kind;
kind!(HashtagType, Hashtag);


/// A specialized Link that represents a hashtag.
///
/// This is just an alias for `Link<MentionType>` because there's no fields inherent to Mention
/// that aren't already present on a Link.
pub type Hashtag = Link<HashtagType>;
