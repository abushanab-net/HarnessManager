//! The 256 words the three verification words are drawn from: short, common, and none sounding like another.

pub(crate) const WORDS: [&str; 256] = [
    "acorn", "actor", "adapt", "agent", "alarm", "album", "alert", "alpha", "amber", "anchor", "angle", "ankle",
    "apple", "apron", "arena", "armor", "arrow", "atlas", "attic", "audio", "autumn", "avenue", "badge", "bagel",
    "baker", "bamboo", "banjo", "barrel", "basil", "basket", "beacon", "beaver", "bench", "berry", "bicycle", "bison",
    "blanket", "blossom", "bonus", "border", "bottle", "bounce", "bracket", "breeze", "brick", "bridge", "bronze", "brush",
    "bubble", "bucket", "buffalo", "bundle", "burger", "butter", "button", "cabin", "cactus", "camel", "camera", "candle",
    "canoe", "canvas", "canyon", "carbon", "carpet", "castle", "cattle", "cedar", "cello", "cherry", "chess", "cider",
    "cinema", "circus", "citrus", "clover", "cobalt", "cocoa", "coffee", "comet", "copper", "coral", "cotton", "cougar",
    "coyote", "cradle", "crater", "crayon", "cricket", "crystal", "cupcake", "curtain", "cushion", "dahlia", "daisy", "dancer",
    "delta", "denim", "desert", "diamond", "dinner", "dolphin", "domino", "donkey", "dragon", "drum", "eagle", "easel",
    "echo", "eclipse", "elbow", "ember", "engine", "falcon", "feather", "ferry", "fiddle", "finch", "flute", "forest",
    "fossil", "fountain", "fox", "galaxy", "garden", "garlic", "gecko", "geyser", "ginger", "glacier", "globe", "goblet",
    "granite", "grape", "gravel", "guitar", "hammer", "harbor", "harvest", "hazel", "helmet", "heron", "honey", "hornet",
    "husky", "igloo", "island", "ivory", "jacket", "jaguar", "jasmine", "jelly", "jigsaw", "jungle", "kayak", "kettle",
    "kitten", "koala", "ladder", "lagoon", "lantern", "laptop", "lemon", "leopard", "lily", "lizard", "lobster", "locket",
    "lotus", "magnet", "mango", "maple", "marble", "meadow", "melon", "meteor", "mirror", "mitten", "monkey", "mosaic",
    "muffin", "nectar", "needle", "nickel", "noodle", "nutmeg", "oasis", "ocean", "olive", "onion", "orange", "orbit",
    "orchid", "otter", "oyster", "paddle", "panda", "papaya", "parrot", "peanut", "pebble", "pencil", "pepper", "piano",
    "pickle", "pigeon", "pillow", "planet", "plum", "pocket", "poppy", "potato", "puzzle", "quartz", "rabbit", "radar",
    "radish", "raven", "ribbon", "rocket", "saddle", "salmon", "sandal", "satin", "scarf", "shadow", "shovel", "silver",
    "sketch", "spider", "sponge", "squash", "statue", "sugar", "summit", "tablet", "teapot", "temple", "thistle", "tiger",
    "timber", "toffee", "tomato", "trumpet", "tulip", "turnip", "velvet", "violin", "walnut", "walrus", "willow", "window",
    "wizard", "yogurt", "zebra", "zipper",
];

#[cfg(test)]
mod tests {
    #[test]
    fn every_word_is_different() {
        let set: std::collections::HashSet<_> = super::WORDS.iter().collect();
        assert_eq!(set.len(), 256);
    }
}
