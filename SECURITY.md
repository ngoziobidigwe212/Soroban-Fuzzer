# Security policy

`soroban-fuzzer` is used to decide whether a contract is safe to deploy, so
a check that silently stops detecting a bug class is a security issue. Please
report suspected vulnerabilities, and checks that miss bugs they claim to
catch, privately using GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)
on this repository instead of opening a public issue.

Include the affected version, a minimal contract and harness that shows the
problem, and the impact you expect. You should get a reply within a week.
