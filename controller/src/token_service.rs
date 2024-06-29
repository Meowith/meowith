use lazy_static::lazy_static;
use rand::rngs::OsRng;
use rand::Rng;

lazy_static! {
    static ref CHARSET: Vec<char> = {
        let mut charset = vec![];
        charset.extend('a'..='z');
        charset.extend('A'..='Z');
        charset.extend('0'..='9');
        charset
    };
}

fn generate_token(len: usize) -> String {
    let mut token = String::new();
    let mut rng: OsRng = Default::default();

    for _ in 0..len {
        let index: usize = rng.gen_range(0..CHARSET.len());
        token.push(CHARSET[index]);
    }

    token
}

pub fn generate_renewal_token() -> String {
    generate_token(64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length() {
        let len = 32;
        let token = generate_token(len);
        assert_eq!(token.len(), len, "Token length should be {}", len);
    }

    #[test]
    fn test_generate_renewal_token_length() {
        let token = generate_renewal_token();
        assert_eq!(token.len(), 64, "Renewal token length should be 64");
    }

    #[test]
    fn test_generate_token_charset() {
        let len = 32;
        let token = generate_token(len);

        for c in token.chars() {
            assert!(CHARSET.contains(&c), "Character {} is not in CHARSET", c);
        }
    }

    #[test]
    fn test_generate_renewal_token_charset() {
        let token = generate_renewal_token();

        for c in token.chars() {
            assert!(CHARSET.contains(&c), "Character {} is not in CHARSET", c);
        }
    }
}
