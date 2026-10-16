//! A text measured and cut and a bound written as the console does, so a refusal reads the same
//! whichever side said it.

/// A text's length as the console counts it, in UTF-16 units.
pub fn length(text: &str) -> usize {
  text.encode_utf16().count()
}

/// The longest head of a text that comes within a number of UTF-16 units, as the console cuts one: a
/// character that would pass the bound is left out whole.
pub fn within(text: &str, units: usize) -> &str {
  let mut spent = 0;
  let end = text
    .char_indices()
    .find(|(_, character)| {
      spent += character.len_utf16();
      spent > units
    })
    .map_or(text.len(), |(at, _)| at);

  &text[..end]
}

/// A bound as the console writes a number, its thousands set apart.
pub fn grouped(number: usize) -> String {
  let digits = number.to_string();
  let mut grouped = String::new();

  for (at, digit) in digits.chars().enumerate() {
    if at > 0 && (digits.len() - at).is_multiple_of(3) {
      grouped.push(',');
    }

    grouped.push(digit);
  }

  grouped
}
