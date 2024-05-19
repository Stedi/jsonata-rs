use glob::glob;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_tests.rs");
    let mut file = fs::File::create(dest_path).unwrap();

    let resources = get_test_resources("tests/testsuite/**/*.json");

    for resource in resources {
        if resource.contains("/skip/") {
            writeln!(
                file,
                r#"
                #[test]
                #[ignore]
                fn test_{}() {{
                    test_case("{}");
                }}
                "#,
                sanitize_filename(&resource),
                resource
            )
            .unwrap();
        } else {
            writeln!(
                file,
                r#"
                #[test]
                fn test_{}() {{
                    test_case("{}");
                }}
                "#,
                sanitize_filename(&resource),
                resource
            )
            .unwrap();
        }
    }
}

fn get_test_resources(pattern: &str) -> Vec<String> {
    glob(pattern)
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok)
        .filter(|path| !path.to_string_lossy().contains("datasets")) // Exclude datasets folder
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}
