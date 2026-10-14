use rice_calc::{BigInt, BigRational, BigUint, Complex, evaluate_exact, prime_factors};

#[test]
fn arbitrary_unsigned_add_multiply_and_divide_are_exact() {
    let left: BigUint = "999999999999999999999999999999999999".parse().unwrap();
    let right: BigUint = "123456789012345678901234567890".parse().unwrap();
    assert_eq!(
        left.add(&right).to_string(),
        "1000000123456789012345678901234567889"
    );
    let product = left.multiply(&right);
    let (quotient, remainder) = product.div_rem(&left).unwrap();
    assert_eq!(quotient, right);
    assert!(remainder.is_zero());
}

#[test]
fn arbitrary_rationals_reduce_and_retain_repeating_values() {
    let third = BigRational::new(
        "1000000000000000000000000000000".parse().unwrap(),
        "3000000000000000000000000000000".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(third.to_string(), "1/3");
    assert_eq!(third.decimal(20), "0.33333333333333333333");
    assert_eq!(third.add(&third).unwrap().to_string(), "2/3");
    let huge: BigRational = "999999999999999999999999999999.125".parse().unwrap();
    assert_eq!(huge.decimal(3), "999999999999999999999999999999.125");
}

#[test]
fn exact_complex_arithmetic_has_no_floating_rounding() {
    let first = Complex::new("1.5".parse().unwrap(), "2".parse().unwrap());
    let second = Complex::new("3".parse().unwrap(), "-4".parse().unwrap());
    let product = first.multiply(&second).unwrap();
    assert_eq!(product.real().to_string(), "25/2");
    assert_eq!(product.imaginary().to_string(), "0");
    assert_eq!(first.conjugate().to_string(), "3/2 - 2i");
}

#[test]
fn signed_big_integer_arithmetic_is_unbounded() {
    let left: BigInt = "-999999999999999999999999999999".parse().unwrap();
    let right: BigInt = "1".parse().unwrap();
    assert_eq!(
        left.subtract(&right).to_string(),
        "-1000000000000000000000000000000"
    );
}

#[test]
fn prime_factorization_returns_multiplicity_and_large_prime_remainder() {
    let value: BigUint = "600851475143".parse().unwrap();
    let factors = prime_factors(&value)
        .unwrap()
        .into_iter()
        .map(|factor| factor.to_string())
        .collect::<Vec<_>>();
    assert_eq!(factors, ["71", "839", "1471", "6857"]);
    let repeated: BigUint = "1048576".parse().unwrap();
    assert_eq!(prime_factors(&repeated).unwrap().len(), 20);
}

#[test]
fn exact_expression_parser_handles_huge_values_rationals_and_factorials() {
    assert_eq!(
        evaluate_exact("999999999999999999999999999999 + 1")
            .unwrap()
            .to_string(),
        "1000000000000000000000000000000"
    );
    assert_eq!(evaluate_exact("1 / 3 + 1 / 6").unwrap().to_string(), "1/2");
    assert_eq!(
        evaluate_exact("(2 + 3)^40").unwrap().to_string(),
        "9094947017729282379150390625"
    );
    assert_eq!(
        evaluate_exact("20!").unwrap().to_string(),
        "2432902008176640000"
    );
}
