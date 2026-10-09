// tests/safety_test.rs
use chelp::models::SafetyLevel;
use chelp::safety::evaluate_command_safety;

#[test]
fn test_destructive_patterns() {
    let cases = vec![
        ("rm -rf /var/log", SafetyLevel::Destructive),
        (
            "Remove-Item -Recurse -Force C:\\temp",
            SafetyLevel::Destructive,
        ),
        ("del /f /s /q *.txt", SafetyLevel::Destructive),
        ("format D: /fs:NTFS", SafetyLevel::Destructive),
        ("dd if=/dev/zero of=/dev/sda", SafetyLevel::Destructive),
        ("DROP DATABASE production;", SafetyLevel::Destructive),
        ("git push origin main --force", SafetyLevel::Destructive),
        ("git reset --hard HEAD~1", SafetyLevel::Destructive),
        ("terraform destroy -auto-approve", SafetyLevel::Destructive),
        ("kubectl delete ns prod", SafetyLevel::Destructive),
        ("kubectl delete namespace staging", SafetyLevel::Destructive),
        ("docker system prune -a", SafetyLevel::Destructive),
        ("conda env remove -n base", SafetyLevel::Destructive),
        ("helm uninstall production-app", SafetyLevel::Destructive),
        ("aws s3 rb s3://my-bucket --force", SafetyLevel::Destructive),
        ("az group delete --name my-rg", SafetyLevel::Destructive),
    ];

    for (cmd, expected) in cases {
        let (level, warning) = evaluate_command_safety(cmd);
        assert_eq!(level, expected, "Failed for command: {}", cmd);
        assert!(warning.is_some(), "Expected warning message for: {}", cmd);
    }
}

#[test]
fn test_safe_and_caution_patterns() {
    assert_eq!(evaluate_command_safety("git status").0, SafetyLevel::Safe);
    assert_eq!(
        evaluate_command_safety("find . -name '*.rs'").0,
        SafetyLevel::Safe
    );
    assert_eq!(evaluate_command_safety("docker ps -a").0, SafetyLevel::Safe);
    assert_eq!(
        evaluate_command_safety("git push origin feature-branch").0,
        SafetyLevel::Caution
    );
}
