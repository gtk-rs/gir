pub trait CaseExt {
    type Owned;

    /// Changes string to snake_case.
    ///
    /// Inserts underscores in-between lowercase and uppercase characters when
    /// they appear in that order and in-between second last and last
    /// uppercase character when going from sequence of three or more
    /// uppercase characters to lowercase.
    ///
    /// Changes the whole string to lowercase.
    fn to_snake(&self) -> Self::Owned;

    /// Changes string to CamelCase.
    ///
    /// Uppercases each character that follows an underscore or is at the
    /// beginning of the string.
    ///
    /// Removes all underscores.
    fn to_camel(&self) -> Self::Owned;
}

impl CaseExt for str {
    type Owned = String;

    fn to_snake(&self) -> Self::Owned {
        let mut s = String::new();
        let mut upper = true;
        let mut upper_count = 0;

        for c in self.chars() {
            let next_upper = if c.is_uppercase() {
                true
            } else if c.is_lowercase() {
                false
            } else {
                upper
            };

            if !upper && next_upper {
                s.push('_');
            } else if upper && !next_upper && upper_count >= 3 {
                let n = s.len() - s.chars().next_back().unwrap().len_utf8();
                s.insert(n, '_');
            }

            s.extend(c.to_lowercase());

            if next_upper {
                upper_count += 1;
                upper = true;
            } else {
                upper_count = 0;
                upper = false;
            }
        }
        s
    }

    fn to_camel(&self) -> Self::Owned {
        let new_length = self.chars().filter(|c| *c != '_').count();
        let mut s = String::with_capacity(new_length);
        let mut under = true;
        for c in self.chars() {
            if under && c.is_lowercase() {
                s.extend(c.to_uppercase());
            } else if c != '_' {
                s.push(c);
            }
            under = c == '_';
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_snake() {
        let cases = [
            ("AtkGObjectAccessible", "atk_gobject_accessible"),
            ("AtkNoOpObject", "atk_no_op_object"),
            ("DConfClient", "dconf_client"),
            ("GCabCabinet", "gcab_cabinet"),
            ("GstA52Dec", "gst_a52_dec"),
            ("GstLameMP3Enc", "gst_lame_mp3_enc"),
            ("GstMpg123AudioDec", "gst_mpg123_audio_dec"),
            ("GstX264Enc", "gst_x264_enc"),
            ("FileIOStream", "file_io_stream"),
            ("IOStream", "io_stream"),
            ("IMContext", "im_context"),
            ("DBusMessage", "dbus_message"),
            ("SoupCookieJarDB", "soup_cookie_jar_db"),
            ("FooBarBaz", "foo_bar_baz"),
            ("aBcDe", "a_bc_de"),
            ("aXXbYYc", "a_xxb_yyc"),
        ];

        for &(input, expected) in &cases {
            assert_eq!(expected, input.to_snake());
        }
    }

    #[test]
    fn to_camel() {
        assert_eq!("foo_bar_baz".to_camel(), "FooBarBaz");
        assert_eq!("_foo".to_camel(), "Foo");
    }
}
