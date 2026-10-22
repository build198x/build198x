"""Checked additions for releasing an existing tag through manual dispatch."""

EVENT = """  workflow_dispatch:
    inputs:
      release_tag:
        description: Existing stable build198x or build198x-adf version tag
        required: true
        type: string
"""
IDENTITY_SCRIPT = """set -euo pipefail
if [[ "$RELEASE_EVENT" == workflow_dispatch ]]; then
  [[ "$RELEASE_TAG" =~ ^build198x(-adf)?-v[0-9]+\\.[0-9]+\\.[0-9]+$ ]] || { echo 'Invalid release tag' >&2; exit 1; }
  tag_commit=$(git rev-parse --verify "refs/tags/${RELEASE_TAG}^{commit}")
  [[ "$tag_commit" == "$(git rev-parse HEAD)" ]] || { echo 'Checkout differs from release tag' >&2; exit 1; }
fi
printf 'commit=%s\\n' "$(git rev-parse HEAD)" >> "$GITHUB_OUTPUT"
"""
IDENTITY_STEP = """      - name: Bind the selected release source
        id: identity
        shell: bash
        env:
          RELEASE_EVENT: ${{ github.event_name }}
          RELEASE_TAG: ${{ inputs.release_tag }}
        run: |
""" + "".join("          " + line + "\n" for line in IDENTITY_SCRIPT.splitlines())

# Every replacement has a checked count. An upstream template change fails closed.
REPLACEMENTS = [
    ("on:\n  pull_request:\n", "on:\n" + EVENT + "  pull_request:\n", 1),
    ("github.ref_name", "(inputs.release_tag || github.ref_name)", 3),
    (
        "          persist-credentials: false\n",
        "          persist-credentials: false\n          ref: ${{ needs.plan.outputs.source }}\n",
        5,
    ),
    (
        "          ref: ${{ needs.plan.outputs.source }}\n          submodules: recursive\n      - name: Install dist\n        # we specify bash",
        "          ref: ${{ inputs.release_tag || github.ref }}\n          submodules: recursive\n      - name: Install dist\n        # we specify bash",
        1,
    ),
    (
        "    outputs:\n      val: ${{ steps.plan.outputs.manifest }}\n",
        "    outputs:\n      source: ${{ steps.identity.outputs.commit }}\n      val: ${{ steps.plan.outputs.manifest }}\n",
        1,
    ),
    (
        "      - name: Install dist\n        # we specify bash",
        IDENTITY_STEP + "      - name: Install dist\n        # we specify bash",
        1,
    ),
    (
        "uses: ./.github/workflows/test-linux-packages.yml\n    with:\n",
        "uses: ./.github/workflows/test-linux-packages.yml\n    with:\n      source_ref: ${{ needs.plan.outputs.source }}\n",
        1,
    ),
    (
        '          RELEASE_COMMIT: "${{ github.sha }}"',
        '          RELEASE_COMMIT: "${{ needs.plan.outputs.source }}"',
        1,
    ),
    (
        "uses: ./.github/workflows/notify-discord.yml\n    with:\n",
        "uses: ./.github/workflows/notify-discord.yml\n    with:\n      release_tag: ${{ needs.plan.outputs.tag }}\n",
        1,
    ),
]


def add_dispatch(text: str) -> str:
    for old, new, count in REPLACEMENTS:
        if text.count(old) != count:
            raise ValueError(
                "cargo-dist template changed; review release dispatch guards"
            )
        text = text.replace(old, new)
    return text
