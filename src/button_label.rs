//! Apply CHK's button-caption casing at the UI boundary, never to stored content.

pub(crate) fn button_label(label: &str) -> String {
    let mut output = String::with_capacity(label.len());
    let mut first_word = true;
    for part in label.split_inclusive(char::is_whitespace) {
        let letters = part
            .chars()
            .filter(|character| character.is_alphabetic())
            .count();
        let capitalize = first_word || letters > 3;
        let mut first_letter = true;
        for character in part.chars() {
            if first_letter && character.is_alphabetic() {
                first_letter = false;
                first_word = false;
                if capitalize {
                    output.extend(character.to_uppercase());
                    continue;
                }
            }
            output.push(character);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::button_label;

    #[test]
    fn button_words_follow_the_length_rule() {
        assert_eq!(button_label("add password"), "Add Password");
        assert_eq!(button_label("Share a login"), "Share a Login");
        assert_eq!(button_label("Save all"), "Save all");
        assert_eq!(button_label("go to my profile"), "Go to my Profile");
        assert_eq!(button_label("Capital letter"), "Capital Letter");
        assert_eq!(button_label("Special character"), "Special Character");
    }

    #[test]
    fn acronyms_whitespace_and_existing_case_are_preserved() {
        assert_eq!(button_label("Scan QR code"), "Scan QR Code");
        assert_eq!(button_label("Open URL"), "Open URL");
        assert_eq!(button_label("  paste\tclipboard\n"), "  Paste\tClipboard\n");
        assert_eq!(button_label("Copy iCloud URL"), "Copy ICloud URL");
        assert_eq!(button_label("Send to NASA"), "Send to NASA");
    }

    #[test]
    fn punctuation_and_unicode_do_not_break_word_casing() {
        assert_eq!(button_label("show (code)"), "Show (Code)");
        assert_eq!(button_label("add (a) key"), "Add (a) key");
        assert_eq!(button_label("copy über"), "Copy Über");
        assert_eq!(button_label("…"), "…");
        assert_eq!(button_label(""), "");
        let label = "Paste Clipboard";
        assert_eq!(button_label(&button_label(label)), label);
    }
}
