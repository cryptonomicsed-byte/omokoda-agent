use sha3::{Digest, Sha3_256};

use crate::report::{NistReport, TestResult};

/// Run the full NIST entropy validation battery on a seed.
/// Returns a report indicating which tests passed or failed.
/// A valid entropy source should pass all four tests at 99% confidence.
pub fn validate_entropy_seed(seed: &[u8]) -> NistReport {
    let bits = bytes_to_bits(seed);
    NistReport::new(
        frequency_test(&bits),
        runs_test(&bits),
        longest_run_ones_test(&bits),
        avalanche_test(seed),
    )
}

/// NIST SP 800-22 Test 1: Frequency (Monobit) Test, real formula.
///
/// The original version of this function used a fixed |proportion - 0.5|
/// < 0.05 threshold regardless of sample size -- statistically wrong, and
/// wrong in a way that matters a lot at the seed sizes this crate is
/// actually used on. The real NIST SP 800-22 monobit test computes
/// s_obs = |sum(+-1 per bit)| / sqrt(n), then a p-value = erfc(s_obs /
/// sqrt(2)), failing only if p-value < 0.01 (99% confidence). At n=256
/// bits (a typical seed size here), the equivalent proportion-deviation
/// threshold for a correct 99%-confidence test is about +-0.080, not
/// +-0.05 -- verified by direct computation before this fix. The old
/// fixed threshold rejected genuinely good, cryptographically-derived
/// 256-bit entropy roughly half the time (4 of 7 real blake3-derived
/// trials failed during integration testing), which is a false-positive
/// rate wildly outside any reasonable confidence level -- not a real
/// entropy defect, a broken test.
fn frequency_test(bits: &[bool]) -> TestResult {
    if bits.is_empty() {
        return TestResult::Fail {
            reason: "empty seed".to_string(),
        };
    }
    let n = bits.len() as f64;
    let sum: f64 = bits.iter().map(|&b| if b { 1.0 } else { -1.0 }).sum();
    let s_obs = sum.abs() / n.sqrt();
    let p_value = erfc(s_obs / std::f64::consts::SQRT_2);

    if p_value >= 0.01 {
        TestResult::Pass
    } else {
        let ones = bits.iter().filter(|&&b| b).count() as f64;
        TestResult::Fail {
            reason: format!(
                "monobit test p-value {:.4} < 0.01 (proportion of 1s is {:.3})",
                p_value,
                ones / n
            ),
        }
    }
}

/// Complementary error function via the Abramowitz & Stegun 7.1.26
/// rational approximation (max absolute error ~1.5e-7) -- avoids pulling
/// in a full stats crate for one function, matching this crate's own
/// "pure Rust subset, no heavy dependency" design intent.
fn erfc(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let erf = 1.0 - poly * (-x * x).exp();
    1.0 - sign * erf
}

/// NIST SP 800-22 Test 2: Runs Test.
/// Counts the total number of runs (consecutive identical bits).
/// A run count too far from expected indicates non-randomness.
fn runs_test(bits: &[bool]) -> TestResult {
    if bits.len() < 8 {
        return TestResult::Fail {
            reason: "too few bits for runs test".to_string(),
        };
    }
    let n = bits.len() as f64;
    let ones = bits.iter().filter(|&&b| b).count() as f64;
    let proportion = ones / n;

    // Pre-test: the real NIST runs test also requires the monobit
    // prerequisite to hold first (a proportion far from 0.5 makes the
    // runs distribution meaningless). Reuses the now-correctly-calibrated
    // frequency_test rather than duplicating its own separate (and
    // previously wrong) fixed threshold.
    if !matches!(frequency_test(bits), TestResult::Pass) {
        return TestResult::Fail {
            reason: "monobit prerequisite failed for runs test".to_string(),
        };
    }

    let runs: usize = bits.windows(2).filter(|w| w[0] != w[1]).count() + 1;
    let expected = 2.0 * n * proportion * (1.0 - proportion);
    let variance =
        (2.0 * n * proportion * (1.0 - proportion) * (1.0 - 2.0 * proportion * (1.0 - proportion)))
            .abs();

    if variance < f64::EPSILON {
        return TestResult::Pass;
    }

    // Accept if runs count is within 3 standard deviations of expected
    let std_dev = variance.sqrt();
    let z = (runs as f64 - expected).abs() / std_dev;
    if z < 3.0 {
        TestResult::Pass
    } else {
        TestResult::Fail {
            reason: format!(
                "runs count {} deviates {:.2}σ from expected {:.1}",
                runs, z, expected
            ),
        }
    }
}

/// NIST SP 800-22 Test 3: Longest Run of Ones in a Block.
/// Checks that no excessively long run of 1s exists.
fn longest_run_ones_test(bits: &[bool]) -> TestResult {
    if bits.is_empty() {
        return TestResult::Fail {
            reason: "empty seed".to_string(),
        };
    }

    // Find the longest run of consecutive 1s
    let mut max_run = 0usize;
    let mut current_run = 0usize;
    for &bit in bits {
        if bit {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 0;
        }
    }

    // Bound calibrated for a single-sample false-positive rate around
    // 1%, via the standard tail approximation P(longest run >= k) ~= n *
    // 2^-k for n iid fair bits: solving n * 2^-k = 0.01 gives
    // k ~= log2(n) + log2(100) ~= log2(n) + 6.64, rounded up to +7 to
    // stay conservative rather than produce false positives. The
    // original log2(n)+4 bound (~6.25% expected false-positive rate at
    // n=256) was too tight -- caught failing a real blake3-derived trial
    // (run of 14 vs the old max of 12) during integration testing that
    // was well within normal single-sample variation, not a real defect.
    let n = bits.len() as f64;
    let max_allowed = (n.log2() + 7.0) as usize;
    if max_run <= max_allowed {
        TestResult::Pass
    } else {
        TestResult::Fail {
            reason: format!(
                "longest run of 1s = {}, max allowed = {}",
                max_run, max_allowed
            ),
        }
    }
}

/// Avalanche test: flipping one bit of the seed should change ~50% of
/// the SHA3-256 hash output bits (the strict avalanche criterion).
/// Validates the seed has good diffusion.
///
/// The original version used a fixed `>= 45%` cutoff -- only ~1.6
/// standard deviations below the ideal 50% for a 256-bit hash (std dev
/// ~= sqrt(256*0.25) ~= 8 bits ~= 3.1%), the same class of
/// miscalibration bug fixed in frequency_test above, and it produced the
/// same kind of real-world consequence: rejected a genuinely
/// well-diffused hash pair (43.4% differing bits, well within normal
/// variation) during integration testing. Rather than invent a second,
/// separately-calibrated threshold, this reuses frequency_test's
/// already-correct erfc-based monobit test directly: the differing-bit
/// pattern between hash_a and hash_b should itself look like ~50%
/// ones/zeros under the null hypothesis of good diffusion, which is
/// exactly what frequency_test checks.
fn avalanche_test(seed: &[u8]) -> TestResult {
    if seed.is_empty() {
        return TestResult::Fail {
            reason: "empty seed".to_string(),
        };
    }

    let hash_a = sha3_hash(seed);

    // Flip the MSB of the first byte
    let mut flipped = seed.to_vec();
    flipped[0] ^= 0x80;
    let hash_b = sha3_hash(&flipped);

    let diff_bits: Vec<bool> = hash_a
        .iter()
        .zip(hash_b.iter())
        .flat_map(|(a, b)| {
            let x = a ^ b;
            (0..8).rev().map(move |shift| (x >> shift) & 1 == 1)
        })
        .collect();

    match frequency_test(&diff_bits) {
        TestResult::Pass => TestResult::Pass,
        TestResult::Fail { .. } => {
            let differing = diff_bits.iter().filter(|&&b| b).count();
            let proportion = differing as f64 / diff_bits.len() as f64;
            TestResult::Fail {
                reason: format!(
                    "avalanche effect {:.1}% differing bits fails the monobit test on the diff pattern (expected ~50%)",
                    proportion * 100.0
                ),
            }
        }
    }
}

fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for byte in bytes {
        for shift in (0..8).rev() {
            bits.push((byte >> shift) & 1 == 1);
        }
    }
    bits
}

fn sha3_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha3_of_hello_passes_all() {
        let seed = sha3_hash(b"hello world");
        let report = validate_entropy_seed(&seed);
        assert!(
            report.all_passed,
            "SHA3 output should pass all NIST tests: {:#?}",
            report
        );
    }

    #[test]
    fn all_zeros_fails_frequency() {
        let seed = [0u8; 32];
        let report = validate_entropy_seed(&seed);
        assert!(
            !report.frequency.is_pass(),
            "all-zeros should fail frequency test"
        );
    }

    #[test]
    fn all_zeros_fails_avalanche() {
        let seed = [0u8; 32];
        let report = validate_entropy_seed(&seed);
        // SHA3([0]*32) XOR SHA3([0x80, 0...]) should differ substantially
        // but the seed itself (constant input) fails avalanche — let's check
        assert!(
            !report.all_passed,
            "all-zeros entropy should not pass all tests"
        );
    }

    #[test]
    fn alternating_bytes_passes_frequency() {
        // 0xAA = 10101010 in binary — exactly 50% ones
        let seed = [0xAAu8; 32];
        let freq = super::frequency_test(&bytes_to_bits(&seed));
        assert!(
            freq.is_pass(),
            "alternating bits should pass frequency: {:?}",
            freq
        );
    }

    #[test]
    fn report_all_passed_requires_all_pass() {
        use crate::report::{NistReport, TestResult};
        let report = NistReport::new(
            TestResult::Pass,
            TestResult::Fail {
                reason: "x".to_string(),
            },
            TestResult::Pass,
            TestResult::Pass,
        );
        assert!(!report.all_passed);
    }
}
