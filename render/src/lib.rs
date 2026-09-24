pub fn greeting() -> &'static str {
    "black-hole renderer"
}

#[cfg(test)]
mod tests {
    #[test]
    fn greets() {
        assert_eq!(super::greeting(), "black-hole renderer");
    }
}
