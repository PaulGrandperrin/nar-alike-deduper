
/// Custom automaton to match store paths.
/// 
/// Since the matching pattern is directly written in code instead of being interpreted from an NFA or DFA, it should be much faster.
/// This customized algorithm should have a better time complexity than a compiled NFA and a much much much better space complexity than a compiled DFA (full/hybrid & sparce/dense),
/// making it faster and more memory efficient than both.

pub struct StorePathAutomaton {
  state: usize,
  overlapping_matchs: bool, // are we matching "/nix/store/nix" until now
}

const OVERLAPPING_SUB_MATCH: &[u8] = b"/nix/store/nix"; // serves 2 purposes: the first 11 bytes to check if the beggining matches and the last 3 bytes to check if we are matching two overlapping matches

impl StorePathAutomaton {
  pub fn new() -> Self {
    Self {
      state: 0,
      overlapping_matchs: true,
    }
  }

  #[inline(always)]
  pub fn next(&mut self, byte: u8) -> bool {
    let restart = match self.state {
      0..=10 => { // matching "/nix/store/"
        if byte == OVERLAPPING_SUB_MATCH[self.state] { // matching "/nix/store/"
          false
        } else if self.state == 5 && byte == b'n' { // matching "/nix/n"
          self.state = 1; // backtrack to "/n"
          false
        } else {
          true
        }
      }
      11..=42 => { // match the 32 bytes long base32 (nix specific) hash 
        if self.overlapping_matchs && self.state < 14 && byte == OVERLAPPING_SUB_MATCH[self.state] { // matching "/nix/store/nix" which fits two overlapping matches
          false
        } else {
          if self.state < 14 { // we don't match "/nix/store/nix" anymore
            self.overlapping_matchs = false;
          }
          match byte {
            b'0'..=b'9' | b'a'..=b'd' /* e */ | b'f'..=b'n' /* o */ | b'p'..=b's' /* t & u */ | b'v'..=b'z' => { // digits (10) + alphabet (26) without eout (-4) = 32
              false
            }
            _ => {
              if self.overlapping_matchs && self.state == 14 && byte == b'/' { // we matched "/nix/store/nix/" so we backtrack to "/nix/"
                self.state = 4;
                false
              } else {
                true
              }
            }
          }
        }
      }
      43 => { // match final "-"
        if byte == b'-' {
          return true;
        } else {
          true
        }
      }
      _ => {
        // unsafe { std::hint::unreachable_unchecked() }
        unreachable!()
      }
    };

    if restart {
      self.overlapping_matchs = true;

      if byte == b'/' {
        self.state = 1;
      } else {
        self.state = 0;
      }
    } else {
      self.state += 1;
    }

    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn helper(bytes: &[u8]) -> bool {
    let mut automaton = StorePathAutomaton::new();
    let mut r = false;
    for byte in bytes {
      r = automaton.next(*byte);
      if r {
        break;
      }
    }
    r
  }

  #[test]
  fn matches() {
    assert_eq!(helper(b"/nix/store/01234567890000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/abcdfghijklmnpqrsvwxyz0000000000-"), true);

    //

    assert_eq!(helper(b"/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"#/nix/store/00000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/ni/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix#/nix/store/00000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store/00000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/00000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/00000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/00000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/00000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/00000000000000000000000000000000-"), true);

    //

    assert_eq!(helper(b"/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"#/nix/store/n0000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/ni/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix#/nix/store/n0000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store/n0000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/n0000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/n0000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/n0000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/n0000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/n0000000000000000000000000000000-"), true);

    //

    assert_eq!(helper(b"/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"#/nix/store/ni000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/ni/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix#/nix/store/ni000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store/ni000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/ni000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/ni000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/ni000000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/ni000000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/ni000000000000000000000000000000-"), true);

    //

    assert_eq!(helper(b"/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"#/nix/store/nix00000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/ni/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix#/nix/store/nix00000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store/nix00000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/nix00000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/nix00000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/nix00000000000000000000000000000-"), true);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/nix00000000000000000000000000000-"), true);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/nix00000000000000000000000000000-"), true);
  }

  #[test]
  fn no_matches() {
    assert_eq!(helper(b"/nix/store/e0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/o0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/u0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/t0000000000000000000000000000000-"), false);

    assert_eq!(helper(b"@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"#@nix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/ni@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix#@nix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#@nix/store/00000000000000000000000000000000-"), false);

    //

    assert_eq!(helper(b"/nix/store/nix/store@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#@nix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#@nix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#@nix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store/@nix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#@nix/store/00000000000000000000000000000000-"), false);

    // 

    assert_eq!(helper(b"/nix/store/nix/store/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/@ix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/@ix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/@ix/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//@ix/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/@ix/store/00000000000000000000000000000000-"), false);

    // 

    assert_eq!(helper(b"/nix/store/nix/store/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/n@x/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/n@x/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/n@x/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//n@x/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/n@x/store/00000000000000000000000000000000-"), false);

    // 

    assert_eq!(helper(b"/nix/store/nix/store/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/ni@/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/ni@/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/ni@/store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//ni@/store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/ni@/store/00000000000000000000000000000000-"), false);

    // 

    assert_eq!(helper(b"/nix/store/nix/store/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/nix@store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix@store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/nix@store/00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//nix@store/00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix@store/00000000000000000000000000000000-"), false);

    //

    assert_eq!(helper(b"/nix/store/nix/store/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store@00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store@00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store@00000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store@00000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store@00000000000000000000000000000000-"), false);

    //

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/@0000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/@0000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/@0000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/@0000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/@0000000000000000000000000000000-"), false);

    //

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/0@000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/0@000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/0@000000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/0@000000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/0@000000000000000000000000000000-"), false);

    //

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/00@00000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/00@00000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/00@00000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/00@00000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/00@00000000000000000000000000000-"), false);

    //

    assert_eq!(helper(b"/nix/store/nix/store/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/stor/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store//nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store/nix/store#/nix/store/000@0000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/stor/nix/store/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/stor/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store//nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/stor/nix/store#/nix/store/000@0000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store//nix/store/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/stor/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store//nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store//nix/store#/nix/store/000@0000000000000000000000000000-"), false);

    assert_eq!(helper(b"/nix/store#/nix/store/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/stor/nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store//nix/store/000@0000000000000000000000000000-"), false);
    assert_eq!(helper(b"/nix/store#/nix/store#/nix/store/000@0000000000000000000000000000-"), false);
  }

}
