//! Free check for the referee: signs in a diff that a test was bent to pass.
//! It cannot tell a cleanup from a cheat; it only points the referee at the lines.

use crate::secrets::Finding;

const ASSERTS: &[&str] = &[
    "assert", "expect(", ".should", "tobe(", "toequal(", "require.", "verify(", "check_eq",
    "ensure!(",
];

const SKIPS: &[&str] = &[
    "#[ignore",
    "@pytest.mark.skip",
    "@pytest.mark.xfail",
    "@unittest.skip",
    "pytest.skip(",
    "it.skip(",
    "describe.skip(",
    "test.skip(",
    "it.todo(",
    "xit(",
    "xdescribe(",
    "@disabled",
    "@ignore",
    "t.skip(",
    "skip=",
    ".skip(",
];

fn is_test_file(path: &str) -> bool {
    let p = path.to_lowercase();
    p.contains("test") || p.contains("spec") || p.ends_with(".rs")
}

/// Scan a unified diff (`git show --unified=0 <commit>`).
pub fn scan_diff(diff: &str) -> Vec<Finding> {
    let mut out = vec![];
    let mut file = String::new();
    let mut old_file = String::new();
    let mut line_no = 0usize;
    let mut removed_asserts = 0usize;
    let mut last_added = String::new();
    let flush = |out: &mut Vec<Finding>, file: &str, n: &mut usize| {
        if *n > 0 {
            out.push(Finding {
                file: file.into(),
                line: 0,
                what: if *n == 1 {
                    "an assert removed"
                } else {
                    "asserts removed"
                },
            });
            *n = 0;
        }
    };
    for l in diff.lines() {
        if let Some(f) = l.strip_prefix("--- a/") {
            flush(&mut out, &file, &mut removed_asserts);
            old_file = f.to_string();
            continue;
        }
        if let Some(f) = l.strip_prefix("+++ ") {
            if f == "/dev/null"
                && is_test_file(&old_file)
                && old_file.to_lowercase().contains("test")
            {
                out.push(Finding {
                    file: old_file.clone(),
                    line: 0,
                    what: "test file deleted",
                });
            }
            file = f.strip_prefix("b/").unwrap_or(f).to_string();
            continue;
        }
        if let Some(h) = l.strip_prefix("@@") {
            line_no = h
                .split('+')
                .nth(1)
                .and_then(|s| s.split([',', ' ']).next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(1);
            continue;
        }
        if let Some(removed) = l.strip_prefix('-') {
            let r = removed.trim().to_lowercase();
            if is_test_file(&file) && !r.starts_with("//") && ASSERTS.iter().any(|a| r.contains(a))
            {
                removed_asserts += 1;
            }
            continue;
        }
        let Some(added) = l.strip_prefix('+') else {
            continue;
        };
        let a = added.trim().to_lowercase();
        if !a.starts_with("//") && !a.starts_with('#') || a.starts_with("#[") {
            if SKIPS.iter().any(|s| a.contains(s)) && is_test_file(&file) {
                out.push(Finding {
                    file: file.clone(),
                    line: line_no,
                    what: "test skipped or ignored",
                });
            }
            let empty_catch = a.contains("catch") && (a.ends_with("{}") || a.ends_with("{ }"));
            let bare_pass = a == "pass" && last_added.starts_with("except");
            let inline_pass = a.starts_with("except") && a.ends_with(": pass");
            if empty_catch || bare_pass || inline_pass {
                out.push(Finding {
                    file: file.clone(),
                    line: line_no,
                    what: "error swallowed",
                });
            }
        }
        last_added = a;
        line_no += 1;
    }
    flush(&mut out, &file, &mut removed_asserts);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bent_tests_are_found() {
        let diff = "\
--- a/tests/width.rs
+++ b/tests/width.rs
@@ -10,2 +10,2 @@
-    assert_eq!(line.width(), 40);
+    assert!(line.width() > 0);
@@ -20,0 +20,1 @@
+#[ignore]
--- a/src/load.py
+++ b/src/load.py
@@ -5,0 +5,2 @@
+    except Exception:
+        pass
--- a/web/app.test.js
+++ /dev/null
@@ -1,3 +0,0 @@
-it('works', () => expect(f()).toBe(1))
";
        let f = scan_diff(diff);
        let what: Vec<_> = f.iter().map(|x| (x.file.as_str(), x.what)).collect();
        assert!(
            what.contains(&("tests/width.rs", "an assert removed")),
            "{what:?}"
        );
        assert!(what.contains(&("tests/width.rs", "test skipped or ignored")));
        assert!(what.contains(&("src/load.py", "error swallowed")));
        assert!(what.contains(&("web/app.test.js", "test file deleted")));
    }

    #[test]
    fn ordinary_changes_pass() {
        let diff = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-    let x = 1;
+    let x = 2;
+    // we skip( nothing here
";
        assert!(scan_diff(diff).is_empty());
    }
}
