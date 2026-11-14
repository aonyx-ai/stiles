//! Items that are test code, beside items that are not

pub fn shipped() {}

#[cfg(not(test))]
pub fn only_outside_tests() {}

#[cfg(all(test, feature = "x"))]
fn only_with_the_feature() {}

#[test]
fn a_test() {}

#[cfg(test)]
mod tests {
    fn helper() -> u8 {
        let local = 1;
        local
    }

    #[test]
    fn works() {
        assert_eq!(super::tests::helper(), 1);
    }
}

mod inner_gated {
    #![cfg(test)]

    fn gated_helper() {}
}
