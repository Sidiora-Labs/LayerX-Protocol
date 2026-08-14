#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$root"

required_files=".editorconfig .gitattributes .gitignore CHANGELOG.md \
CONTRIBUTING.md LICENSE LICENSE_NOTICE.md README.md SECURITY.md SUPPORT.md \
docs/QUALIFICATION.md .github/workflows/ci.yml"
for required_file in $required_files; do
    if [ ! -s "$required_file" ]; then
        echo "required publication file is missing or empty: $required_file" >&2
        exit 1
    fi
done

while IFS= read -r required_ignore; do
    if ! rg -Fqx -- "$required_ignore" .gitignore; then
        echo "required ignore rule is missing: $required_ignore" >&2
        exit 1
    fi
done <<'EOF'
/build/
/build-*/
/.codegraph/
/.logs/
/cache/
.env
*.pem
*.key
*.vsix
EOF

large_file=$(find . \
    \( -path './.git' -o -path './.codegraph' -o -path './.logs' -o \
       -path './build' -o -path './build-*' -o -path './cache' -o \
       -path './broadcast' -o -path './out' \) -prune -o \
    -type f ! -name '*.vsix' -size +50M -print -quit)
if [ -n "$large_file" ]; then
    echo "publishable file exceeds 50 MiB: $large_file" >&2
    exit 1
fi

symlinks=$(find . \
    \( -path './.git' -o -path './.codegraph' -o -path './.logs' -o \
       -path './build' -o -path './build-*' -o -path './cache' -o \
       -path './broadcast' -o -path './out' \) -prune -o \
    -type l -print)
if [ -n "$symlinks" ]; then
    echo "symbolic links require explicit publication review:" >&2
    printf '%s\n' "$symlinks" >&2
    exit 1
fi

audit_rg() {
    rg --hidden -n \
        --glob '!.git/**' --glob '!.codegraph/**' --glob '!.logs/**' \
        --glob '!build/**' --glob '!build-*/**' --glob '!cache/**' \
        --glob '!broadcast/**' --glob '!out/**' --glob '!*.vsix' "$@"
}

private_refs='(/root/|147\.93\.139\.18)'
if audit_rg --glob '!tools/ci/public-repo-audit.sh' "$private_refs" .; then
    echo "private workspace or infrastructure reference found" >&2
    exit 1
fi

secret_shapes='(-----BEGIN (RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{36,}|xox[baprs]-[A-Za-z0-9-]{20,}|sk_live_[A-Za-z0-9]{20,}|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,})'
if audit_rg -i --glob '!tools/ci/public-repo-audit.sh' "$secret_shapes" .; then
    echo "secret-shaped material found in publication set" >&2
    exit 1
fi

for script in tools/*.sh tools/ci/*.sh; do
    sh -n "$script"
done

if command -v go >/dev/null 2>&1; then
    (cd spec/specgen && go run . -root "$root" -check)
else
    echo "go is required to verify generated specification artifacts" >&2
    exit 1
fi

echo "public repository audit passed"
