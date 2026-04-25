# Third-party attribution — @dayseam/website

## Brand marks — Simple Icons (CC0)

The connector brand marks rendered on the landing page (hero animation and
connector grid) are sourced from the [Simple Icons](https://simpleicons.org)
collection, which is released under
[CC0 1.0 Universal (public-domain dedication)](https://creativecommons.org/publicdomain/zero/1.0/).

All marks remain the trademarks of their respective owners and are used here
in the classic "we connect to X" nominative-fair-use sense, not as a claim of
endorsement or affiliation.

| Connector       | Simple Icons slug  | Path location in this repo               |
| --------------- | ------------------ | ---------------------------------------- |
| GitHub          | `github`           | `src/data/connectors.ts` — `GITHUB_PATH` |
| GitLab          | `gitlab`           | `src/data/connectors.ts` — `GITLAB_PATH` |
| Jira            | `jira`             | `src/data/connectors.ts` — `JIRA_PATH`   |
| Confluence      | `confluence`       | `src/data/connectors.ts` — `CONFLUENCE_PATH` |
| Git             | `git`              | `src/data/connectors.ts` — `GIT_PATH`    |
| Slack           | `slack`            | `src/data/connectors.ts` — `SLACK_PATH`  |
| Microsoft Teams | `microsoftteams`   | `src/data/connectors.ts` — `TEAMS_PATH`  |
| Linear          | `linear`           | `src/data/connectors.ts` — `LINEAR_PATH` |
| Microsoft Word  | `microsoftword`    | `src/data/connectors.ts` — `WORD_PATH`   |
| Microsoft Excel | `microsoftexcel`   | `src/data/connectors.ts` — `EXCEL_PATH`  |

The first five (GitHub, GitLab, Jira, Confluence, Git) mirror the paths used
in-app at `apps/desktop/src/components/ConnectorLogo.tsx`. If Simple Icons
updates a mark upstream, change BOTH files in the same PR — there is no CI
gate enforcing the consistency today.

## Dayseam brand mark — "Convergence"

The Dayseam mark (`public/dayseam-mark.svg`, nav logo, favicon, og:image) is
© Dayseam and is **not** CC0-licensed. Do not reuse it outside of Dayseam-
related projects. See `../../docs/brand/README.md` for usage rules.
