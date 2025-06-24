# Branch Protection Rules

This document outlines the recommended branch protection rules for the `main` branch.

## Required Status Checks

The following GitHub Actions workflows should pass before merging:

### Critical Checks
- `lint / lint success`
- `unit / unit success`
- `integration / integration success`
- `e2e / e2e`
- `lint-actions / actionlint`

### Optional but Recommended
- `docker / build` (for releases)

## Settings

1. **Require a pull request before merging**
   - Require approvals: 1
   - Dismiss stale pull request approvals when new commits are pushed
   - Require review from CODEOWNERS

2. **Require status checks to pass before merging**
   - Require branches to be up to date before merging
   - Status checks listed above

3. **Require conversation resolution before merging**

4. **Require signed commits** (optional but recommended)

5. **Include administrators** (optional, depends on team preference)

6. **Restrict who can push to matching branches**
   - Add team/users who can push directly (for emergency fixes)

## Setting Up

To apply these rules:

1. Go to Settings → Branches in your GitHub repository
2. Add a rule for the `main` branch
3. Configure the settings as described above
4. Save changes

## Merge Queue (Optional)

Consider enabling GitHub's merge queue for better handling of multiple PRs:
- Maximum PRs to merge: 5
- Minimum PRs to merge: 1
- Wait time: 5 minutes